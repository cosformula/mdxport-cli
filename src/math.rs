/// Convert LaTeX math to Typst math syntax using tex2typst-rs.
///
/// Falls back to raw passthrough if conversion fails.
pub fn latex_to_typst(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    tex2typst_rs::tex2typst(trimmed).unwrap_or_else(|_| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_variables() {
        let result = latex_to_typst("E = mc^2");
        assert!(result.contains("m") && result.contains("c"));
        assert!(!result.contains("mc")); // should be separated
    }

    #[test]
    fn greek() {
        let result = latex_to_typst("\\alpha + \\beta");
        assert!(result.contains("alpha"));
        assert!(result.contains("beta"));
    }

    #[test]
    fn frac_and_sqrt() {
        let result = latex_to_typst("\\frac{1}{\\sqrt{n+1}}");
        // tex2typst-rs outputs frac or / notation
        assert!(result.contains("sqrt") || result.contains("root"));
    }

    #[test]
    fn integral() {
        let result = latex_to_typst("\\int_a^b f(x) dx");
        assert!(result.contains("int") || result.contains("integral"));
    }

    #[test]
    fn mathbb() {
        let result = latex_to_typst("\\mathbb{R}");
        assert!(result.contains("RR") || result.contains("bb(R)"));
    }

    #[test]
    fn complex_expression() {
        let result = latex_to_typst(
            "\\widehat{f}(\\xi)=\\int_{-\\infty}^{\\infty} f(x) e^{-i 2 \\pi \\xi x} d x",
        );
        assert!(!result.is_empty());
        assert!(result.contains("hat") || result.contains("widehat"));
    }
}
