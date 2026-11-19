use portable_size_witness::exercise;

#[test]
fn portable_module_witness_returns_live_results() {
    assert_eq!(exercise(), 0xede3_ec1b);
}
