pub const ABI_VERSION: &str = "1.0.0";

pub fn is_compatible(expected: &str, actual: &str) -> bool {
    let expected_major = extract_major(expected);
    let actual_major = extract_major(actual);
    match (expected_major, actual_major) {
        (Some(e), Some(a)) => e == a,
        _ => false,
    }
}

pub fn validate_version(expected: &str, actual: &str) -> Result<(), String> {
    if is_compatible(expected, actual) {
        Ok(())
    } else {
        Err(format!(
            "ABI version mismatch: expected major from '{}', got '{}'",
            expected, actual
        ))
    }
}

pub fn extract_major(version: &str) -> Option<u64> {
    version.split('.').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_version_compatible() {
        assert!(is_compatible("1.0.0", "1.0.0"));
    }

    #[test]
    fn test_same_major_compatible() {
        assert!(is_compatible("1.0.0", "1.5.0"));
        assert!(is_compatible("1.5.0", "1.0.0"));
    }

    #[test]
    fn test_different_major_incompatible() {
        assert!(!is_compatible("1.0.0", "2.0.0"));
        assert!(!is_compatible("2.0.0", "1.0.0"));
    }

    #[test]
    fn test_invalid_version_incompatible() {
        assert!(!is_compatible("1.0.0", "invalid"));
        assert!(!is_compatible("invalid", "1.0.0"));
    }

    #[test]
    fn test_empty_version_incompatible() {
        assert!(!is_compatible("1.0.0", ""));
        assert!(!is_compatible("", "1.0.0"));
    }

    #[test]
    fn test_validate_version_ok() {
        assert!(validate_version("1.0.0", "1.2.3").is_ok());
    }

    #[test]
    fn test_validate_version_err() {
        assert!(validate_version("1.0.0", "2.0.0").is_err());
    }

    #[test]
    fn test_extract_major() {
        assert_eq!(extract_major("1.0.0"), Some(1));
        assert_eq!(extract_major("2.5.1"), Some(2));
        assert_eq!(extract_major("0.9.0"), Some(0));
        assert_eq!(extract_major("invalid"), None);
        assert_eq!(extract_major(""), None);
    }

    #[test]
    fn test_abi_version_constant_valid() {
        let major = extract_major(ABI_VERSION);
        assert!(major.is_some());
    }
}
