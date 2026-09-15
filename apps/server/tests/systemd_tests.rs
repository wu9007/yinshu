#[test]
fn unit_runs_as_yinshu_and_uses_notify() {
    let unit = include_str!("../packaging/systemd/yinshu.service");
    assert!(unit.contains("User=yinshu"));
    assert!(unit.contains("Group=yinshu"));
    assert!(unit.contains("Type=notify"));
    assert!(unit.contains("RuntimeDirectory=yinshu"));
    assert!(unit.contains("ExecStart=/usr/bin/yinshu serve"));
}
