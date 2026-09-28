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
    if [[ $# -gt 0 ]]; then
        command mend "$@"
        return $?
    fi
    local cmd="$_MEND_LAST_CMD"
    if [[ -z "$cmd" ]]; then
        echo "mend: No previous command executed in this session." >&2
        return 1
    fi
    local code="${_MEND_LAST_EXIT:-1}"
    local err=""
    if [[ -f "$_MEND_LOG" ]]; then
        err="$(tail -n 30 "$_MEND_LOG" 2>/dev/null)"
    fi
    command mend fix --command "$cmd" --exit-code "$code" --stderr "$err"
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
    if [[ $# -gt 0 ]]; then
        command mend "$@"
        return $?
    fi
    local cmd="$_MEND_LAST_CMD"
    if [[ -z "$cmd" ]]; then
        echo "mend: No previous command executed in this session." >&2
        return 1
    fi
    local code="${_MEND_LAST_EXIT:-1}"
    local err=""
    if [[ -f "$_MEND_LOG" ]]; then
        err="$(tail -n 30 "$_MEND_LOG" 2>/dev/null)"
    fi
    command mend fix --command "$cmd" --exit-code "$code" --stderr "$err"
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
    if test (count $argv) -gt 0
        command mend $argv
        return $status
    end
    set -l cmd "$_MEND_LAST_CMD"
    if test -z "$cmd"
        echo "mend: No previous command executed in this session." >&2
        return 1
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zsh_hook_uses_command_mend() {
        let hook = generate_shell_hook("zsh");
        assert!(!hook.contains("\\mend"));
        assert!(!hook.contains("fc -ln"));
        assert!(hook.contains("command mend fix"));
        assert!(hook.contains("command mend \"$@\""));
        assert!(hook.contains("No previous command executed in this session"));
    }

    #[test]
    fn test_bash_hook_uses_command_mend() {
        let hook = generate_shell_hook("bash");
        assert!(!hook.contains("\\mend"));
        assert!(hook.contains("command mend fix"));
        assert!(hook.contains("command mend \"$@\""));
        assert!(hook.contains("No previous command executed in this session"));
    }

    #[test]
    fn test_fish_hook_uses_command_mend() {
        let hook = generate_shell_hook("fish");
        assert!(!hook.contains("\\mend"));
        assert!(!hook.contains("$history[1]"));
        assert!(hook.contains("command mend fix"));
        assert!(hook.contains("command mend $argv"));
        assert!(hook.contains("No previous command executed in this session"));
    }
}
