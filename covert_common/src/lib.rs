#![warn(missing_docs)]
#![feature(slice_as_chunks)]

use bincode_aes::{create_key, with_key, BincodeCryptor};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{collections::VecDeque, error::Error};

#[derive(Deserialize, Serialize)]
struct CovertPacket {
    syn: u16,
    want: u16,
    last: bool,
    payload: Vec<u8>,
}

impl CovertPacket {
    fn new(syn: u16, last: bool, payload: &[u8]) -> Self {
        Self {
            syn,
            want: 0,
            last,
            payload: payload.to_vec(),
        }
    }
    fn set_want(&mut self, val: u16) {
        self.want = val;
    }
}

pub struct CovertChannel<T>
where
    T: DeserializeOwned + Serialize,
{
    out_bound_packet_cache: VecDeque<CovertPacket>,
    in_bound_packet_cache: VecDeque<CovertPacket>,
    message_cache: VecDeque<T>,
    cryptor: BincodeCryptor,
    out_count: u16,
    out_syn: u16,
    in_syn: u16,
}

impl<T> CovertChannel<T>
where
    T: DeserializeOwned + Serialize,
{
    pub fn new(key: Vec<u8>) -> Result<CovertChannel<T>, Box<dyn Error>> {
        let cryptor = with_key(create_key(key).or(Err("bad key"))?);
        Ok(CovertChannel {
            out_bound_packet_cache: VecDeque::new(),
            in_bound_packet_cache: VecDeque::new(),
            message_cache: VecDeque::new(),
            cryptor: cryptor,
            out_count: 0,
            out_syn: 0,
            in_syn: 0,
        })
    }

    pub fn get_message(&mut self) -> Option<T> {
        self.message_cache.pop_front()
    }
    pub fn put_message(&mut self, msg: T) -> Result<(), Box<dyn Error>> {
        let res = self.cryptor.serialize(&msg)?;
        let (chunks, remainder) = res.as_chunks::<20>();
        for chunk in chunks {
            let new_packet = CovertPacket::new(self.out_count, false, chunk.as_slice());
            self.out_count += 1;
            self.out_bound_packet_cache.push_back(new_packet);
        }
        let last_packet = CovertPacket::new(self.out_count, true, remainder);
        self.out_bound_packet_cache.push_back(last_packet);
        self.out_count += 1;
        Ok(())
    }
    pub fn get_packet(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        if self.out_bound_packet_cache.len() == 0 {
            self.out_bound_packet_cache.push_front(CovertPacket::new(
                self.out_count,
                false,
                &[],
            ));
            self.out_count += 1;
        }
        let out = self.out_bound_packet_cache.front_mut().unwrap();
        out.set_want(self.in_syn);
        self.cryptor.serialize(out)
    }

    pub fn put_packet(&mut self, pkt: &[u8]) -> Result<bool, Box<dyn Error>> {
        let in_packet = self
            .cryptor
            .deserialize::<CovertPacket>(&mut pkt.to_vec())?;

        // Clear packets in the out_cache that have been confirmed
        self.out_syn = Ord::max(self.out_syn, in_packet.want);
        self.out_bound_packet_cache
            .retain(|item| item.syn >= self.out_syn);

        // Append to the in_cache if this is the next packet needed else drop it
        if in_packet.syn == self.in_syn {
            self.in_syn += 1;
            let is_last = in_packet.last;
            self.in_bound_packet_cache.push_back(in_packet);
            if is_last {
                let mut payload = self
                    .in_bound_packet_cache
                    .drain(..)
                    .map(|i| i.payload)
                    .flatten()
                    .collect::<Vec<u8>>();
                let in_message = self.cryptor.deserialize::<T>(&mut payload)?;
                self.message_cache.push_back(in_message);
                return Ok(true);
            } else {
                return Ok(false);
            }
        }
        return Ok(false);
    }
}

#[cfg(test)]
mod test;
