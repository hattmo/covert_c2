#[cfg(test)]
mod test {

    use crate::CovertChannel;
    use rand::prelude::*;

    #[test]
    fn test() {
        let mut chan1: CovertChannel<Vec<u32>, 2> = CovertChannel::new([5; 32]);
        let mut chan2: CovertChannel<Vec<u32>, 3> = CovertChannel::new([5; 32]);
        chan1.put_message(vec![20; 50], 1);
        chan1.put_message(vec![10; 50], 2);
        for _ in 0..20 {
            let pkt = chan1.get_packet(1);
            println!("{:?}", pkt.len());
            if random() {
                chan2.put_packet(pkt.as_slice()).expect("can put packet");
            };
            let pkt = chan2.get_packet(1);
            println!("{:?}", pkt.len());
            if random() {
                chan1.put_packet(pkt.as_slice()).expect("can put packet");
            }
            let pkt = chan1.get_packet(2);
            if random() {
                chan2.put_packet(pkt.as_slice()).expect("can put packet");
            };
            let pkt = chan2.get_packet(2);
            if random() {
                chan1.put_packet(pkt.as_slice()).expect("can put packet");
            }
        }
        println!("{:?}", chan2.get_message(1));
        println!("{:?}", chan2.get_message(2));
    }
}
