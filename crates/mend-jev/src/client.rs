use mend_core::{ActionStrategy, DecisionPlan, FailureReason, JevResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum JevClientError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),
    #[error("Evaluation response parse error: {0}")]
    ParseError(String),
    #[error("Jev API returned error status {0}: {1}")]
    ApiError(u16, String),
    #[error("Server returned missing answer for question '{0}'")]
    MissingAnswer(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JevApiEvaluationRequest {
    pub domain: String,
    pub context: String,
    pub questions: Vec<JevQuestionPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JevQuestionPayload {
    pub id: String,
    #[serde(rename = "type")]
    pub question_type: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JevApiEvaluationResponse {
    pub evaluations: Vec<JevAnswerEvaluation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JevAnswerEvaluation {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_option: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
}

#[derive(Clone)]
pub struct JevClient {
    client: Client,
    endpoint: String,
    api_key: Option<String>,
}

impl JevClient {
    pub fn new(endpoint: impl Into<String>, api_key: Option<String>) -> Self {
        let client = Client::builder()
            .tcp_keepalive(Some(Duration::from_secs(60)))
            .pool_idle_timeout(Some(Duration::from_secs(90)))
            .pool_max_idle_per_host(10)
            .timeout(Duration::from_millis(1500))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            endpoint: endpoint.into(),
            api_key,
        }
    }

    pub async fn evaluate(&self, plan: &DecisionPlan) -> Result<JevResponse, JevClientError> {
        let payload = JevApiEvaluationRequest {
            domain: plan.domain.clone(),
            context: plan.context_text.clone(),
            questions: plan
                .questions
                .iter()
                .map(|q| match q {
                    mend_core::QuestionSpec::Choice {
                        id,
                        prompt,
                        options,
                    } => JevQuestionPayload {
                        id: id.clone(),
                        question_type: "choice".to_string(),
                        prompt: prompt.clone(),
                        options: Some(options.clone()),
                        min: None,
                        max: None,
                    },
                    mend_core::QuestionSpec::Noul {
                        id,
                        prompt,
                        min,
                        max,
                    } => JevQuestionPayload {
                        id: id.clone(),
                        question_type: "noul".to_string(),
                        prompt: prompt.clone(),
                        options: None,
                        min: Some(*min),
                        max: Some(*max),
                    },
                })
                .collect(),
        };

        let mut req = self.client.post(&self.endpoint).json(&payload);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(JevClientError::ApiError(status, body));
        }

        let eval_resp: JevApiEvaluationResponse = resp
            .json()
            .await
            .map_err(|e| JevClientError::ParseError(e.to_string()))?;

        self.decode_response(eval_resp)
    }

    fn decode_response(
        &self,
        resp: JevApiEvaluationResponse,
    ) -> Result<JevResponse, JevClientError> {
        let mut failure_reason = FailureReason::Unknown;
        let mut failure_reason_confidence = 0.5;
        let mut remediation_action = ActionStrategy::Abort;
        let mut action_confidence = 0.5;
        let mut destructive_risk = 0.5;

        for eval in resp.evaluations {
            match eval.id.as_str() {
                "failure_reason" => {
                    if let Some(opt) = eval.selected_option {
                        failure_reason = parse_failure_reason(&opt);
                    }
                    if let Some(conf) = eval.confidence {
                        failure_reason_confidence = conf;
                    }
                }
                "remediation_action" => {
                    if let Some(opt) = eval.selected_option {
                        remediation_action = parse_remediation_action(&opt);
                    }
                    if let Some(conf) = eval.confidence {
                        action_confidence = conf;
                    }
                }
                "destructive_risk" => {
                    if let Some(score) = eval.score {
                        destructive_risk = score.clamp(0.0, 1.0);
                    }
                }
                _ => {}
            }
        }

        Ok(JevResponse {
            failure_reason,
            failure_reason_confidence,
            remediation_action,
            action_confidence,
            destructive_risk,
        })
    }
}

fn parse_failure_reason(s: &str) -> FailureReason {
    match s {
        "PERMISSION_DENIED" => FailureReason::PermissionDenied,
        "COMMAND_NOT_FOUND" => FailureReason::CommandNotFound,
        "GIT_NO_UPSTREAM" => FailureReason::GitNoUpstream,
        "GIT_NON_FAST_FORWARD" => FailureReason::GitNonFastForward,
        "GIT_UNCOMMITTED_CHANGES" => FailureReason::GitUncommittedChanges,
        "MISSING_PACKAGE_OR_BINARY" => FailureReason::MissingPackageOrBinary,
        "NO_SUCH_FILE_OR_DIRECTORY" => FailureReason::NoSuchFileOrDirectory,
        "PERMISSION_NOT_EXECUTABLE" => FailureReason::PermissionNotExecutable,
        "DAEMON_NOT_RUNNING" => FailureReason::DaemonNotRunning,
        _ => FailureReason::Unknown,
    }
}

fn parse_remediation_action(s: &str) -> ActionStrategy {
    match s {
        "PREPEND_SUDO" => ActionStrategy::PrependSudo,
        "GIT_SET_UPSTREAM" => ActionStrategy::GitSetUpstream,
        "GIT_PULL_REBASE" => ActionStrategy::GitPullRebase,
        "GIT_STASH_POP" => ActionStrategy::GitStashPop,
        "GIT_CHECKOUT_BRANCH" => ActionStrategy::GitCheckoutBranch,
        "APT_INSTALL_PACKAGE" => ActionStrategy::AptInstallPackage,
        "BREW_INSTALL_PACKAGE" => ActionStrategy::BrewInstallPackage,
        "CARGO_INSTALL_PACKAGE" => ActionStrategy::CargoInstallPackage,
        "PATH_CORRECTION" => ActionStrategy::PathCorrection,
        "MAKE_DIRECTORY" => ActionStrategy::MakeDirectory,
        "CHMOD_EXECUTABLE" => ActionStrategy::ChmodExecutable,
        "DOCKER_START_DAEMON" => ActionStrategy::DockerStartDaemon,
        _ => ActionStrategy::Abort,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_jev_client_eval_success() {
        let mock_server = MockServer::start().await;

        let response_body = serde_json::json!({
            "evaluations": [
                {
                    "id": "failure_reason",
                    "selected_option": "GIT_NO_UPSTREAM",
                    "confidence": 0.95
                },
                {
                    "id": "remediation_action",
                    "selected_option": "GIT_SET_UPSTREAM",
                    "confidence": 0.96
                },
                {
                    "id": "destructive_risk",
                    "score": 0.05
                }
            ]
        });

        Mock::given(method("POST"))
            .and(path("/evaluate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        let client = JevClient::new(format!("{}/evaluate", mock_server.uri()), None);
        let plan = DecisionPlan {
            domain: "git_operations".into(),
            context_text: "git push failed".into(),
            questions: vec![],
        };

        let result = client.evaluate(&plan).await.expect("Evaluation succeeded");
        assert_eq!(result.failure_reason, FailureReason::GitNoUpstream);
        assert_eq!(result.remediation_action, ActionStrategy::GitSetUpstream);
        assert_eq!(result.action_confidence, 0.96);
        assert_eq!(result.destructive_risk, 0.05);
    }
}
