#[cfg(test)]
mod test {
    use crate::CovertChannel;
    use rand::prelude::*;
    #[test]
    fn test() {
        let mut chan1: CovertChannel<u32> = CovertChannel::new(vec![5; 32]).unwrap();
        let mut chan2: CovertChannel<u32> = CovertChannel::new(vec![5; 32]).unwrap();
        chan1.put_message(50).unwrap();
        chan1.put_message(32).unwrap();
        for _ in 0..50{
            let pkt1 = chan1.get_packet().unwrap();
            if random() {
                chan2.put_packet(pkt1.as_slice()).unwrap();
            };
            let pkt2 = chan2.get_packet().unwrap();
            if random() {
                chan1.put_packet(pkt2.as_slice()).unwrap();
            }
        }
        assert_eq!(chan2.get_message().unwrap(), 50);
        assert_eq!(chan2.get_message().unwrap(), 32);
    }
}
