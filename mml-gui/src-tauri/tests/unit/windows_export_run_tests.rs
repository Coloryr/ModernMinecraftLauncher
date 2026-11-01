use super::*;

#[test]
fn export_permit_releases_after_error() {
    let busy = AtomicBool::new(false);
    let failed = || -> Result<(), String> {
        let _permit = ExportPermit::acquire(&busy)?;
        assert!(ExportPermit::acquire(&busy).is_err());
        Err("invalid path".to_string())
    };
    assert!(failed().is_err());
    assert!(ExportPermit::acquire(&busy).is_ok());
}

