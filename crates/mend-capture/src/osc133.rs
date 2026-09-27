/// OSC 133 Shell Integration protocol block parser.
/// Matches:
/// - \x1b]133;C\x07: Command execution / output start
/// - \x1b]133;D;<exit-code>\x07: Command finished with exit code
#[derive(Debug, Clone, PartialEq)]
pub struct Osc133Block {
    pub exit_code: i32,
    pub output: Vec<u8>,
}

pub fn parse_osc133_blocks(stream: &[u8]) -> Vec<Osc133Block> {
    let mut blocks = Vec::new();
    let mut i = 0;
    let n = stream.len();

    let start_seq = b"\x1b]133;C\x07";
    let end_prefix = b"\x1b]133;D";

    while i < n {
        // Look for start sequence
        if let Some(start_idx) = find_subsequence(&stream[i..], start_seq) {
            let content_start = i + start_idx + start_seq.len();
            // Look for end sequence after content_start
            if let Some(end_rel_idx) = find_subsequence(&stream[content_start..], end_prefix) {
                let content_end = content_start + end_rel_idx;
                let payload_slice = &stream[content_start..content_end];

                // Parse exit code between end_prefix and terminal BEL (\x07) or ST (\x1b\\)
                let meta_start = content_end + end_prefix.len();
                let mut meta_end = meta_start;
                while meta_end < n && stream[meta_end] != 0x07 && stream[meta_end] != 0x1b {
                    meta_end += 1;
                }

                let meta_str = String::from_utf8_lossy(&stream[meta_start..meta_end]);
                // format can be ";0" or "0"
                let exit_code = meta_str
                    .trim_start_matches(';')
                    .trim()
                    .parse::<i32>()
                    .unwrap_or(0);

                blocks.push(Osc133Block {
                    exit_code,
                    output: payload_slice.to_vec(),
                });

                i = meta_end + 1;
                continue;
            }
        }
        break;
    }

    blocks
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_osc133_block() {
        let stream = b"\x1b]133;C\x07error: command failed\n\x1b]133;D;1\x07";
        let blocks = parse_osc133_blocks(stream);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].exit_code, 1);
        assert_eq!(blocks[0].output, b"error: command failed\n");
    }
}
