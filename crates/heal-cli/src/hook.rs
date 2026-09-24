pub fn generate_shell_hook(shell: &str) -> String {
    match shell.to_lowercase().as_str() {
        "zsh" => r#"
# jev-heal zsh integration
_JEV_HEAL_LOG="${TMPDIR:-/tmp}/jev-heal-$USER-last.log"

_jev_heal_preexec() {
    local first_word="${1%% *}"
    if [[ "$first_word" != "fix" && "$first_word" != "fuck" && "$first_word" != "jev-heal" ]]; then
        export _JEV_HEAL_LAST_CMD="$1"
        # Truncate and open log output capture
        : > "$_JEV_HEAL_LOG" 2>/dev/null
        exec 3>&2
        exec 2> >(tee -a "$_JEV_HEAL_LOG" >&3)
    fi
}

_jev_heal_precmd() {
    local exit_code=$?
    # Restore stderr if redirected
    if { true >&3; } 2>/dev/null; then
        exec 2>&3 3>&-
    fi
    if [[ "$_JEV_HEAL_LAST_CMD" != "fix"* && "$_JEV_HEAL_LAST_CMD" != "fuck"* ]]; then
        export _JEV_HEAL_LAST_EXIT=$exit_code
    fi
}

autoload -Uz add-zsh-hook
add-zsh-hook preexec _jev_heal_preexec
add-zsh-hook precmd _jev_heal_precmd

fuck() {
    local cmd="$_JEV_HEAL_LAST_CMD"
    if [[ -z "$cmd" ]]; then
        cmd="$(fc -ln -1 2>/dev/null | sed 's/^[ ]*//')"
    fi
    local code="${_JEV_HEAL_LAST_EXIT:-1}"
    local err=""
    if [[ -f "$_JEV_HEAL_LOG" ]]; then
        err="$(tail -n 30 "$_JEV_HEAL_LOG" 2>/dev/null)"
    fi
    jev-heal fix --command "$cmd" --exit-code "$code" --stderr "$err"
}
alias fix=fuck
"#
        .to_string(),

        "bash" => r#"
# jev-heal bash integration
_JEV_HEAL_LOG="${TMPDIR:-/tmp}/jev-heal-$USER-last.log"

_jev_heal_prompt_command() {
    local exit_code=$?
    local last_hist=$(history 1 | sed 's/^[ ]*[0-9]*[ ]*//')
    local first_word="${last_hist%% *}"
    if [[ "$first_word" != "fix" && "$first_word" != "fuck" && "$first_word" != "jev-heal" ]]; then
        export _JEV_HEAL_LAST_EXIT=$exit_code
        export _JEV_HEAL_LAST_CMD="$last_hist"
    fi
}

PROMPT_COMMAND="_jev_heal_prompt_command; $PROMPT_COMMAND"

fuck() {
    local cmd="$_JEV_HEAL_LAST_CMD"
    if [[ -z "$cmd" ]]; then
        cmd="$(history 1 | sed 's/^[ ]*[0-9]*[ ]*//')"
    fi
    local code="${_JEV_HEAL_LAST_EXIT:-1}"
    local err=""
    if [[ -f "$_JEV_HEAL_LOG" ]]; then
        err="$(tail -n 30 "$_JEV_HEAL_LOG" 2>/dev/null)"
    fi
    jev-heal fix --command "$cmd" --exit-code "$code" --stderr "$err"
}
alias fix=fuck
"#
        .to_string(),

        "fish" => r#"
# jev-heal fish integration
set -g _JEV_HEAL_LOG "/tmp/jev-heal-$USER-last.log"

function _jev_heal_postexec --on-event fish_postexec
    set -l first_word (string split ' ' $argv[1])[1]
    if test "$first_word" != "fix" -a "$first_word" != "fuck" -a "$first_word" != "jev-heal"
        set -gx _JEV_HEAL_LAST_CMD $argv[1]
        set -gx _JEV_HEAL_LAST_EXIT $status
    end
end

function fuck
    set -l cmd "$_JEV_HEAL_LAST_CMD"
    if test -z "$cmd"
        set cmd "$history[1]"
    end
    set -l err ""
    if test -f "$_JEV_HEAL_LOG"
        set err (tail -n 30 "$_JEV_HEAL_LOG" 2>/dev/null)
    end
    jev-heal fix --command "$cmd" --exit-code "$_JEV_HEAL_LAST_EXIT" --stderr "$err"
end
alias fix=fuck
"#
        .to_string(),

        _ => format!("# Unsupported shell: {}", shell),
    }
}
