//! Integration test skeleton — EP-001. Real tests added at EP-003+.
use pb_core::Fix32;

#[test]
fn fix32_still_works_from_integration_context() {
    assert_eq!((Fix32::from_int(2) * Fix32::from_int(3)).to_int_floor(), 6);
}
