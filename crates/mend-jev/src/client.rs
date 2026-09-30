use mend_core::{ActionStrategy, DecisionPlan, FailureReason, JevResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
pub struct SystemOneApiRequest {
    pub state: String,
    pub model: String,
    pub questions: HashMap<String, SystemOneQuestionPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SystemOneQuestionPayload {
    Choice {
        instructions: String,
        criteria: HashMap<String, String>,
    },
    Score {
        instructions: String,
        criteria: Vec<String>,
    },
    Noul {
        instructions: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        criteria: Option<HashMap<String, String>>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemOneApiResponse {
    pub model: Option<String>,
    pub answers: HashMap<String, SystemOneAnswerPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemOneAnswerPayload {
    #[serde(rename = "type")]
    pub answer_type: Option<String>,
    pub choice: Option<String>,
    pub selected_option: Option<String>,
    pub confidence: Option<f64>,
    pub score: Option<f64>,
    pub noul: Option<f64>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UnifiedApiResponse {
    SystemOne(SystemOneApiResponse),
    Legacy(JevApiEvaluationResponse),
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
            .timeout(Duration::from_millis(3000))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            endpoint: endpoint.into(),
            api_key,
        }
    }

    pub async fn evaluate(&self, plan: &DecisionPlan) -> Result<JevResponse, JevClientError> {
        let is_systemone =
            self.endpoint.contains("systemone") || !self.endpoint.contains("evaluate");

        let mut req = if is_systemone {
            let mut questions = HashMap::new();
            for q in &plan.questions {
                match q {
                    mend_core::QuestionSpec::Choice {
                        id,
                        prompt,
                        options,
                    } => {
                        let mut criteria = HashMap::new();
                        for opt in options {
                            criteria.insert(opt.clone(), opt.clone());
                        }
                        questions.insert(
                            id.clone(),
                            SystemOneQuestionPayload::Choice {
                                instructions: prompt.clone(),
                                criteria,
                            },
                        );
                    }
                    mend_core::QuestionSpec::Noul { id, prompt, .. } => {
                        questions.insert(
                            id.clone(),
                            SystemOneQuestionPayload::Score {
                                instructions: prompt.clone(),
                                criteria: vec![
                                    "Safe read-only or configuration".into(),
                                    "Low risk inspection".into(),
                                    "Moderate state alteration".into(),
                                    "Dangerous state deletion or overwrite".into(),
                                ],
                            },
                        );
                    }
                }
            }
            let payload = SystemOneApiRequest {
                state: plan.context_text.clone(),
                model: "jev-latest".to_string(),
                questions,
            };
            self.client.post(&self.endpoint).json(&payload)
        } else {
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
            self.client.post(&self.endpoint).json(&payload)
        };

        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(JevClientError::ApiError(status, body));
        }

        let raw_text = resp.text().await?;
        let unified: UnifiedApiResponse = serde_json::from_str(&raw_text)
            .map_err(|e| JevClientError::ParseError(format!("{}: {}", e, raw_text)))?;

        self.decode_unified_response(unified)
    }

    fn decode_unified_response(
        &self,
        unified: UnifiedApiResponse,
    ) -> Result<JevResponse, JevClientError> {
        match unified {
            UnifiedApiResponse::SystemOne(resp) => {
                let mut failure_reason = FailureReason::Unknown;
                let mut failure_reason_confidence = 0.5;
                let mut remediation_action = ActionStrategy::Abort;
                let mut action_confidence = 0.5;
                let mut destructive_risk = 0.5;

                if let Some(ans) = resp.answers.get("failure_reason") {
                    let opt = ans.choice.as_ref().or(ans.selected_option.as_ref());
                    if let Some(o) = opt {
                        failure_reason = parse_failure_reason(o);
                    }
                    if let Some(conf) = ans.confidence {
                        failure_reason_confidence = conf;
                    }
                }

                if let Some(ans) = resp.answers.get("remediation_action") {
                    let opt = ans.choice.as_ref().or(ans.selected_option.as_ref());
                    if let Some(o) = opt {
                        remediation_action = parse_remediation_action(o);
                    }
                    if let Some(conf) = ans.confidence {
                        action_confidence = conf;
                    }
                }

                if let Some(ans) = resp.answers.get("destructive_risk") {
                    if let Some(score) = ans.score.or(ans.noul) {
                        destructive_risk = score.clamp(0.0, 1.0);
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
            UnifiedApiResponse::Legacy(resp) => {
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
    }
}

fn parse_failure_reason(s: &str) -> FailureReason {
    s.parse().unwrap_or(FailureReason::Unknown)
}

fn parse_remediation_action(s: &str) -> ActionStrategy {
    s.parse().unwrap_or(ActionStrategy::Abort)
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

    #[tokio::test]
    async fn test_systemone_client_eval_success() {
        let mock_server = MockServer::start().await;

        let response_body = serde_json::json!({
            "model": "jev-1.13.0",
            "answers": {
                "failure_reason": {
                    "type": "choice",
                    "choice": "COMMAND_NOT_FOUND",
                    "confidence": 0.98
                },
                "remediation_action": {
                    "type": "choice",
                    "choice": "BREW_INSTALL_PACKAGE",
                    "confidence": 0.92
                },
                "destructive_risk": {
                    "type": "score",
                    "score": 0.04,
                    "confidence": 0.99
                }
            }
        });

        Mock::given(method("POST"))
            .and(path("/v1/systemone"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server)
            .await;

        let client = JevClient::new(format!("{}/v1/systemone", mock_server.uri()), None);
        let plan = DecisionPlan {
            domain: "system_cli".into(),
            context_text: "docker ps failed".into(),
            questions: vec![
                mend_core::QuestionSpec::Choice {
                    id: "failure_reason".into(),
                    prompt: "Classify".into(),
                    options: vec!["COMMAND_NOT_FOUND".into()],
                },
                mend_core::QuestionSpec::Choice {
                    id: "remediation_action".into(),
                    prompt: "Remediate".into(),
                    options: vec!["BREW_INSTALL_PACKAGE".into()],
                },
                mend_core::QuestionSpec::Noul {
                    id: "destructive_risk".into(),
                    prompt: "Risk".into(),
                    min: 0.0,
                    max: 1.0,
                },
            ],
        };

        let result = client
            .evaluate(&plan)
            .await
            .expect("SystemOne evaluation succeeded");
        assert_eq!(result.failure_reason, FailureReason::CommandNotFound);
        assert_eq!(
            result.remediation_action,
            ActionStrategy::BrewInstallPackage
        );
        assert_eq!(result.action_confidence, 0.92);
        assert_eq!(result.destructive_risk, 0.04);
    }

    #[test]
    fn test_parse_remediation_action_all_variants() {
        for action in ActionStrategy::ALL {
            let parsed = parse_remediation_action(action.as_str());
            assert_eq!(&parsed, action, "Failed for {}", action.as_str());
        }
        assert_eq!(
            parse_remediation_action("NON_EXISTENT"),
            ActionStrategy::Abort
        );
    }

    #[test]
    fn test_parse_failure_reason_all_variants() {
        for reason in FailureReason::ALL {
            let parsed = parse_failure_reason(reason.as_str());
            assert_eq!(&parsed, reason, "Failed for {}", reason.as_str());
        }
        assert_eq!(parse_failure_reason("NON_EXISTENT"), FailureReason::Unknown);
    }
}
