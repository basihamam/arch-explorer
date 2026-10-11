use std::process::Command;

pub fn compute_checksums(path: &str) -> (String, String) {
    let md5 = match Command::new("md5sum").arg(path).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .split_whitespace().next().unwrap_or("?").to_string(),
        Err(_) => "md5sum bulunamadı".into(),
    };
    let sha256 = match Command::new("sha256sum").arg(path).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .split_whitespace().next().unwrap_or("?").to_string(),
        Err(_) => "sha256sum bulunamadı".into(),
    };
    (md5, sha256)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_compute_checksums() {
        let path = "/tmp/test_checksum_sample.txt";
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(b"hello world").unwrap();
        f.sync_all().unwrap();

        let (md5, sha256) = compute_checksums(path);
        assert_eq!(md5, "5eb63bbbe01eeed093cb22bb8f5acdc3");
        assert_eq!(sha256, "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");

        let _ = std::fs::remove_file(path);
    }
}
