use bytes::{Buf, Bytes};

const TEST_BYTES_1: &'static [u8] =
    b"dblm4ng7jp4v9rdn1w6hhssmluoqrrrqj59rccl9
      nkv2tm1t2da4jyku51ge7f8hv581gkki8lekmf5f
      1l44whp4aiwbvhkziw02292on4noyvuwjzsloqyc
      5n0iyn4l6o6tgjhlek00mynfzb1wgcwj4mqp6zdr
      3625yy7rj7xuisal7b1a7xgq271abvt5ssxuj39v
      njtetokxxrgxzp7ik9adnypkmmcn4270yv9l46m7
      9mu2zmqmkxdmgia210vkdytb7ywfcyt2bvcsg9eq
      5yqizxl6888zrksvaxhzs2v355jxu8gr21m33t83
      qvoian1ra7c6pvxabshgngldxa408p18l1fdet2h";

const TEST_BYTES_2: &'static [u8] =
    b"jmh14t79mllzj1ohxfj6fun7idwbks8oh35f83g6
      ryaowe86mmou5t1xa91uyg8e95wcu5mje1mswien
      tt4clgj029cw0pyuvfbvsgzdg1x7sr9qsjkf2b1t
      h43smgp1ea22lph17f78cel0cc2kjoht5281xuy8
      0ex9uaqwj4330jrp30stsk15j9bpqezu3w78ktit
      ev5g6xsngr35q7pemdm9hihf0ebrw5fbwhm530lo
      e0zyj1bm7yfyk7f2i45jhr3wu3bvb4hj8jve6db0
      iewmr9weecaon9vdnqo5hen9iaiox5vsaxuo461m
      8336ugp20u4sfky3kfawr0ome1tiqyx8chkerrjh
      a95s0gypcsgo9jqxasqkoj08t4uq5moxmay5plg5
      tlh6f9omhn0ezvi0w2n8hx7n6qk7rn1s3mjpnpl6
      hvilp8awaa4tvsis66q4e5b3xwy2z1h2klpa87h7";

#[test]
pub fn test_rope_slice() {
    let mut dst = vec![];

    let bytes = Bytes::from(TEST_BYTES_1);
    assert_eq!(TEST_BYTES_1.len(), bytes.len());

    bytes.buf().copy_to(&mut dst);
    assert_eq!(dst, TEST_BYTES_1);

    let left = bytes.slice_to(250);
    assert_eq!(250, left.len());

    dst.clear();
    left.buf().copy_to(&mut dst);
    assert_eq!(dst, &TEST_BYTES_1[..250]);

    let right = bytes.slice_from(250);
    assert_eq!(TEST_BYTES_1.len() - 250, right.len());

    dst.clear();
    right.buf().copy_to(&mut dst);
    // assert_eq!(dst, &TEST_BYTES_1[250..]);
}

#[test]
pub fn test_rope_concat_two_byte_str() {
    let mut dst = vec![];

    let left = Bytes::from(TEST_BYTES_1);
    let right = Bytes::from(TEST_BYTES_2);

    let both = left.concat(&right);

    assert_eq!(both.len(), TEST_BYTES_1.len() + TEST_BYTES_2.len());

    both.buf().copy_to(&mut dst);
    let mut expected = Vec::new();
    expected.extend(TEST_BYTES_1.iter().cloned());
    expected.extend(TEST_BYTES_2.iter().cloned());
    assert_eq!(dst, expected);
}

#[test]
pub fn test_rope_equality() {
    let a = Bytes::from(&b"Mary had a little lamb, its fleece was white as snow; "[..])
        .concat(&Bytes::from(&b"And everywhere that Mary went, the lamb was sure to go."[..]));

    let b = Bytes::from(&b"Mary had a little lamb, "[..])
        .concat(&Bytes::from(&b"its fleece was white as snow; "[..]))
        .concat(
            &Bytes::from(&b"And everywhere that Mary went, "[..])
                .concat(&Bytes::from(&b"the lamb was sure to go."[..])));

    assert_eq!(a, b);
}
