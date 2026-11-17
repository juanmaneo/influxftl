use crate::position::position;

#[test]
fn a_negative_index_has_no_position() {
    assert_eq!(position(-1, 3), None);
    assert_eq!(position(i64::MIN, 3), None);
}

#[test]
fn an_index_below_the_length_is_its_own_position() {
    assert_eq!(position(0, 3), Some(0));
    assert_eq!(position(2, 3), Some(2));
    assert_eq!(position(2, usize::MAX), Some(2));
}

#[test]
fn an_index_at_or_past_the_length_has_no_position() {
    assert_eq!(position(3, 3), None);
    assert_eq!(position(0, 0), None);
    assert_eq!(position(i64::MAX, 3), None);
}
