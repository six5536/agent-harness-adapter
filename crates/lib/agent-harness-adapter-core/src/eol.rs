//! Line endings: the writers and the regions module read and write LF.

/// CRLF to LF.
pub(crate) fn normalise(text: &str) -> String {
    text.replace("\r\n", "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_reads_as_lf_and_a_lone_cr_stays() {
        assert_eq!(normalise("x\r\ny\r\n"), "x\ny\n");
        assert_eq!(normalise("x\ry\n"), "x\ry\n");
    }
}
