use democutter::cut;
use expect_test::{expect_file, ExpectFile};
use std::fs;

fn snapshot(path: &str, expect: ExpectFile) {
    let file = fs::read(path).unwrap();
    let output = cut(&file, 30000, 50000);
    let output_md5 = md5::compute(&output);

    expect.assert_eq(&format!("{:x}", output_md5));
}

#[test]
fn snapshot_gully() {
    snapshot(
        "test_data/gully.dem",
        expect_file!["../test_data/gully_cut.md5"],
    );
}
