//! Lightweight Unicode rendering for inline and display LaTeX math snippets.

/// Formats a raw `$...$` or `$$...$$` LaTeX math slice into a compact Unicode display string.
pub fn format_latex_math(raw: &str) -> String {
    let mut s = raw.trim();
    if s.starts_with("$$") && s.ends_with("$$") && s.len() >= 4 {
        s = &s[2..s.len() - 2];
    } else if s.starts_with('$') && s.ends_with('$') && s.len() >= 2 {
        s = &s[1..s.len() - 1];
    }
    let s = s.trim();

    if !s.bytes().any(|b| matches!(b, b'\\' | b'^' | b'_' | b'{')) {
        return s.to_string();
    }

    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                let mut end = i + 1;
                while end < s.len() {
                    let Some(ch) = s[end..].chars().next() else {
                        break;
                    };
                    if ch.is_alphabetic() {
                        end += ch.len_utf8();
                    } else {
                        break;
                    }
                }
                if end > i + 1 {
                    let cmd = &s[i + 1..end];
                    if let Some(replacement) = latex_symbol(cmd) {
                        out.push_str(replacement);
                        i = end;
                        continue;
                    }
                    if cmd == "frac" {
                        let mut cursor = end;
                        let num = read_braced_group(s, &mut cursor);
                        let den = read_braced_group(s, &mut cursor);
                        i = cursor;
                        if !num.is_empty() || !den.is_empty() {
                            out.push_str(num);
                            out.push('/');
                            out.push_str(den);
                            continue;
                        }
                        out.push('\\');
                        i = i.min(bytes.len());
                        continue;
                    }
                }
                out.push('\\');
                i += 1;
            }
            b'^' => {
                i += 1;
                push_script_arg(s, &mut i, &mut out, to_superscript);
            }
            b'_' => {
                i += 1;
                push_script_arg(s, &mut i, &mut out, to_subscript);
            }
            b'{' | b'}' => {
                i += 1;
            }
            _ => {
                let ch = s[i..].chars().next().expect("valid UTF-8 slice");
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    out
}

fn read_braced_group<'a>(s: &'a str, i: &mut usize) -> &'a str {
    if s.as_bytes().get(*i) != Some(&b'{') {
        return "";
    }
    *i += 1;
    let start = *i;
    while *i < s.len() && s.as_bytes()[*i] != b'}' {
        let ch = s[*i..].chars().next().expect("valid UTF-8 slice");
        *i += ch.len_utf8();
    }
    let group = &s[start..*i];
    if s.as_bytes().get(*i) == Some(&b'}') {
        *i += 1;
    }
    group
}

fn push_script_arg(s: &str, i: &mut usize, out: &mut String, map: fn(char) -> char) {
    if s.as_bytes().get(*i) == Some(&b'{') {
        *i += 1;
        while *i < s.len() && s.as_bytes()[*i] != b'}' {
            let ch = s[*i..].chars().next().expect("valid UTF-8 slice");
            out.push(map(ch));
            *i += ch.len_utf8();
        }
        if s.as_bytes().get(*i) == Some(&b'}') {
            *i += 1;
        }
    } else if let Some(ch) = s.get(*i..).and_then(|tail| tail.chars().next()) {
        out.push(map(ch));
        *i += ch.len_utf8();
    }
}

fn latex_symbol(cmd: &str) -> Option<&'static str> {
    match cmd {
        "alpha" => Some("α"),
        "beta" => Some("β"),
        "gamma" => Some("γ"),
        "delta" => Some("δ"),
        "epsilon" | "varepsilon" => Some("ε"),
        "zeta" => Some("ζ"),
        "eta" => Some("η"),
        "theta" | "vartheta" => Some("θ"),
        "iota" => Some("ι"),
        "kappa" => Some("κ"),
        "lambda" => Some("λ"),
        "mu" => Some("μ"),
        "nu" => Some("ν"),
        "xi" => Some("ξ"),
        "pi" | "varpi" => Some("π"),
        "rho" | "varrho" => Some("ρ"),
        "sigma" | "varsigma" => Some("σ"),
        "tau" => Some("τ"),
        "upsilon" => Some("υ"),
        "phi" | "varphi" => Some("φ"),
        "chi" => Some("χ"),
        "psi" => Some("ψ"),
        "omega" => Some("ω"),
        "Gamma" => Some("Γ"),
        "Delta" => Some("Δ"),
        "Theta" => Some("Θ"),
        "Lambda" => Some("Λ"),
        "Xi" => Some("Ξ"),
        "Pi" => Some("Π"),
        "Sigma" => Some("Σ"),
        "Phi" => Some("Φ"),
        "Psi" => Some("Ψ"),
        "Omega" => Some("Ω"),
        "times" => Some("×"),
        "cdot" => Some("·"),
        "pm" => Some("±"),
        "mp" => Some("∓"),
        "div" => Some("÷"),
        "neq" | "ne" => Some("≠"),
        "leq" | "le" => Some("≤"),
        "geq" | "ge" => Some("≥"),
        "approx" => Some("≈"),
        "equiv" => Some("≡"),
        "infty" => Some("∞"),
        "in" => Some("∈"),
        "notin" => Some("∉"),
        "subset" => Some("⊂"),
        "subseteq" => Some("⊆"),
        "cup" => Some("∪"),
        "cap" => Some("∩"),
        "emptyset" => Some("∅"),
        "forall" => Some("∀"),
        "exists" => Some("∃"),
        "nabla" => Some("∇"),
        "partial" => Some("∂"),
        "sum" => Some("∑"),
        "prod" => Some("∏"),
        "int" => Some("∫"),
        "to" | "rightarrow" => Some("→"),
        "leftarrow" => Some("←"),
        "Rightarrow" => Some("⇒"),
        "Leftarrow" => Some("⇐"),
        "iff" | "Leftrightarrow" => Some("⇔"),
        "sqrt" => Some("√"),
        "quad" | "qquad" => Some("  "),
        _ => None,
    }
}

fn to_superscript(c: char) -> char {
    match c {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        '+' => '⁺',
        '-' => '⁻',
        '=' => '⁼',
        '(' => '⁽',
        ')' => '⁾',
        'n' => 'ⁿ',
        'i' => 'ⁱ',
        'x' => 'ˣ',
        _ => c,
    }
}

fn to_subscript(c: char) -> char {
    match c {
        '0' => '₀',
        '1' => '₁',
        '2' => '₂',
        '3' => '₃',
        '4' => '₄',
        '5' => '₅',
        '6' => '₆',
        '7' => '₇',
        '8' => '₈',
        '9' => '₉',
        '+' => '₊',
        '-' => '₋',
        '=' => '₌',
        '(' => '₍',
        ')' => '₎',
        'a' => 'ₐ',
        'e' => 'ₑ',
        'i' => 'ᵢ',
        'o' => 'ₒ',
        'u' => 'ᵤ',
        'x' => 'ₓ',
        'n' => 'ₙ',
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::format_latex_math;

    #[test]
    fn formats_greek_scripts_and_fractions() {
        assert_eq!(format_latex_math("$x^2 + y_{i+1}$"), "x² + yᵢ₊₁");
        assert_eq!(
            format_latex_math("$$\\alpha \\cdot \\frac{1}{2} \\neq \\infty$$"),
            "α · 1/2 ≠ ∞"
        );
        assert_eq!(format_latex_math("$E = mc$"), "E = mc");
    }
}
