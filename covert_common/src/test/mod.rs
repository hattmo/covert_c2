#[cfg(test)]
mod test {
    use crate::CovertChannel;
    use aes::{
        cipher::{generic_array::GenericArray, BlockDecrypt, BlockEncrypt, KeyInit},
        Aes128, Aes256,
    };

    use cipher::block_padding::{Padding, Pkcs7, Iso10126};
    use rand::prelude::*;
    #[test]
    fn test() {
        let mut chan1: CovertChannel<u32> = CovertChannel::new(vec![5; 32]);
        let mut chan2: CovertChannel<u32> = CovertChannel::new(vec![5; 32]);
        chan1.put_message(50);
        chan1.put_message(32);
        for _ in 0..50 {
            let pkt1 = chan1.get_packet();
            if random() {
                chan2.put_packet(pkt1.as_slice()).unwrap();
            };
            let pkt2 = chan2.get_packet();
            if random() {
                chan1.put_packet(pkt2.as_slice()).unwrap();
            }
        }
        assert_eq!(chan2.get_message().unwrap(), 50);
        assert_eq!(chan2.get_message().unwrap(), 32);
    }

    #[test]
    fn test_crypto() {
        // let key = arr![u8; 16,24,24,24,24,24,35,34,16,24,24,24,24,24,35,5];
        let key = GenericArray::from([40u8; 16]);
        let buf = vec![42u8; 99];
        let engine = Aes128::new(&key);
        let (parts, end) = buf.as_chunks::<15>();
        for part in parts {
            let res = engine.encrypt_padded_vec::<Iso10126>(part);
            println!("{:?} -- len: {:?}", res, res.len());
        }
        let res = engine.encrypt_padded_vec::<Iso10126>(end);
        println!("{:?} -- extra len: {:?}", res, res.len());
    }
}
