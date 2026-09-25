pub fn generate_shell_hook(shell: &str) -> String {
    match shell.to_lowercase().as_str() {
        "zsh" => r#"
# mend zsh integration
_MEND_LOG="${TMPDIR:-/tmp}/mend-$USER-last.log"

_mend_preexec() {
    local first_word="${1%% *}"
    if [[ "$first_word" != "mend" && "$first_word" != "fix" && "$first_word" != "fuck" ]]; then
        export _MEND_LAST_CMD="$1"
        : > "$_MEND_LOG" 2>/dev/null
        exec 3>&2
        exec 2> >(tee -a "$_MEND_LOG" >&3)
    fi
}

_mend_precmd() {
    local exit_code=$?
    if { true >&3; } 2>/dev/null; then
        exec 2>&3 3>&-
    fi
    if [[ "$_MEND_LAST_CMD" != "mend"* && "$_MEND_LAST_CMD" != "fix"* && "$_MEND_LAST_CMD" != "fuck"* ]]; then
        export _MEND_LAST_EXIT=$exit_code
    fi
}

autoload -Uz add-zsh-hook
add-zsh-hook preexec _mend_preexec
add-zsh-hook precmd _mend_precmd

mend() {
    local cmd="$_MEND_LAST_CMD"
    if [[ -z "$cmd" ]]; then
        cmd="$(fc -ln -1 2>/dev/null | sed 's/^[ ]*//')"
    fi
    local code="${_MEND_LAST_EXIT:-1}"
    local err=""
    if [[ -f "$_MEND_LOG" ]]; then
        err="$(tail -n 30 "$_MEND_LOG" 2>/dev/null)"
    fi
    \mend fix --command "$cmd" --exit-code "$code" --stderr "$err"
}
alias fix=mend
alias fuck=mend
"#
        .to_string(),

        "bash" => r#"
# mend bash integration
_MEND_LOG="${TMPDIR:-/tmp}/mend-$USER-last.log"

_mend_prompt_command() {
    local exit_code=$?
    local last_hist=$(history 1 | sed 's/^[ ]*[0-9]*[ ]*//')
    local first_word="${last_hist%% *}"
    if [[ "$first_word" != "mend" && "$first_word" != "fix" && "$first_word" != "fuck" ]]; then
        export _MEND_LAST_EXIT=$exit_code
        export _MEND_LAST_CMD="$last_hist"
    fi
}

PROMPT_COMMAND="_mend_prompt_command; $PROMPT_COMMAND"

mend() {
    local cmd="$_MEND_LAST_CMD"
    if [[ -z "$cmd" ]]; then
        cmd="$(history 1 | sed 's/^[ ]*[0-9]*[ ]*//')"
    fi
    local code="${_MEND_LAST_EXIT:-1}"
    local err=""
    if [[ -f "$_MEND_LOG" ]]; then
        err="$(tail -n 30 "$_MEND_LOG" 2>/dev/null)"
    fi
    \mend fix --command "$cmd" --exit-code "$code" --stderr "$err"
}
alias fix=mend
alias fuck=mend
"#
        .to_string(),

        "fish" => r#"
# mend fish integration
set -g _MEND_LOG "/tmp/mend-$USER-last.log"

function _mend_postexec --on-event fish_postexec
    set -l first_word (string split ' ' $argv[1])[1]
    if test "$first_word" != "mend" -a "$first_word" != "fix" -a "$first_word" != "fuck"
        set -gx _MEND_LAST_CMD $argv[1]
        set -gx _MEND_LAST_EXIT $status
    end
end

function mend
    set -l cmd "$_MEND_LAST_CMD"
    if test -z "$cmd"
        set cmd "$history[1]"
    end
    set -l err ""
    if test -f "$_MEND_LOG"
        set err (tail -n 30 "$_MEND_LOG" 2>/dev/null)
    end
    command mend fix --command "$cmd" --exit-code "$_MEND_LAST_EXIT" --stderr "$err"
end
alias fix=mend
alias fuck=mend
"#
        .to_string(),

        _ => format!("# Unsupported shell: {}", shell),
    }
}
