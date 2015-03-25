use bytes::*;

#[test]
pub fn test_debug_short_str_valid_ascii() {
    let b = Bytes::from_slice(b"abcdefghij234");
    let d = format!("{:?}", b);

    assert_eq!(d, "Bytes[len=13; abcdefghij234]");
}

#[test]
pub fn test_debug_long_str_valid_ascii() {
    let s = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. \
             Duis volutpat eros in gravida malesuada. Phasellus lobortis \
             maximus cursus. Praesent tristique orci non purus porta \
             dapibus. Ut ut commodo risus, sed semper felis. Phasellus \
             bibendum dui nunc, ac pharetra dui viverra a. Nunc imperdiet \
             sed nulla ut condimentum. In hac habitasse platea dictumst. \
             Interdum et malesuada fames ac ante ipsum primis in faucibus. \
             Sed facilisis dictum malesuada. Sed tempor odio ullamcorper mi \
             iaculis, eu tempus diam semper. Vivamus pulvinar metus ac erat \
             aliquet aliquam.";

    let b = Bytes::from_slice(s.as_bytes());

    let d = format!("{:?}", b);

    assert_eq!(d, "Bytes[len=556; Lorem ipsum dolor sit amet, \
                   consectetur adipiscing elit. Duis volutpat \
                   eros in gravida malesuada. Phasellus \
                   lobortis maximus cur ... ]");
}

#[test]
pub fn test_short_string_invalid_ascii() {
    let b = Bytes::from_slice(b"foo\x00bar\xFFbaz");
    let d = format!("{:?}", b);

    println!("{:?}", b);

    assert_eq!(d, "Bytes[len=11; foo\\x00bar\\xFFbaz]");
}
