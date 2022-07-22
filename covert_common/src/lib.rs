#![warn(missing_docs)]
#![feature(slice_as_chunks)]

//! Helper utilities for creating [external c2][1] systems for [cobaltstrike][2].
//!
//! ![C2](https://i.ibb.co/Cszd81H/externalc2.png)
//!
//!
//!
//![1]: https://hstechdocs.helpsystems.com/manuals/cobaltstrike/current/userguide/content/topics/listener-infrastructue_external-c2.htm
//! [2]: https://www.cobaltstrike.com/

use anyhow::{anyhow, Result};
use bincode_aes::{create_key, with_key, BincodeCryptor};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Deserialize, Serialize, Debug)]
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

/// Establishes a covert channel handler.  this struct enables sending series of messages
/// that are broken into small encrypted packets.  messages are put into the channel with
/// put_message and then sent with get_packet.  In the reverse direction packets are
/// read into the channel with put_packets and coverted to messages with get_message.
/// packets need to be sent in both directions to "syn/ack" to confirm messages are
/// transmitted in full.

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
    /// Create a new covert channel with the provided 32 byte aes key.
    pub fn new(key: Vec<u8>) -> CovertChannel<T> {
        let cryptor = with_key(create_key(key).expect("Bad key"));
        CovertChannel {
            out_bound_packet_cache: VecDeque::new(),
            in_bound_packet_cache: VecDeque::new(),
            message_cache: VecDeque::new(),
            cryptor: cryptor,
            out_count: 0,
            out_syn: 0,
            in_syn: 0,
        }
    }

    /// get a complete message T from this channel.  if there are now messages sent completely
    /// yet then the Option is None.
    pub fn get_message(&mut self) -> Option<T> {
        self.message_cache.pop_front()
    }

    /// send a message through this channel.  
    pub fn put_message(&mut self, msg: T) -> () {
        let res = self.cryptor.serialize(&msg).expect("Failed to serialize");
        let (chunks, remainder) = res.as_chunks::<20>();
        for chunk in chunks {
            let new_packet = CovertPacket::new(self.out_count, false, chunk.as_slice());
            self.out_count += 1;
            self.out_bound_packet_cache.push_back(new_packet);
        }
        let last_packet = CovertPacket::new(self.out_count, true, remainder);
        self.out_bound_packet_cache.push_back(last_packet);
        self.out_count += 1;
    }

    /// get the next packet that needs to be sent for this channel.  Even if there are
    /// no messages to be sent a call to get_packet with return successfully.  empty packets
    /// still contain synchronizing information and should be sent regularly.  additionally
    /// the receiving end of the channel accounts for packets that contain no message data.
    pub fn get_packet(&mut self) -> Vec<u8> {
        if self.out_bound_packet_cache.len() == 0 {
            self.out_bound_packet_cache.push_front(CovertPacket::new(
                self.out_count,
                false,
                &[],
            ));
            self.out_count += 1;
        }
        let out = self
            .out_bound_packet_cache
            .front_mut()
            .expect("Should always have a packet");
        out.set_want(self.in_syn);
        self.cryptor
            .serialize(out)
            .expect("Should always serialize")
    }

    /// put packets into this channel to be decoded.  When a complete packet is ready to
    /// be read from the channel put packet will contain true in the Ok result.
    pub fn put_packet(&mut self, pkt: &[u8]) -> Result<bool> {
        let in_packet = self
            .cryptor
            .deserialize::<CovertPacket>(&mut pkt.to_vec())
            .or(Err(anyhow!("Not a valid packet")))?;

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
                let mut payload: Vec<u8> = self
                    .in_bound_packet_cache
                    .drain(..)
                    .map(|i| i.payload)
                    .flatten()
                    .collect();
                let in_message = self
                    .cryptor
                    .deserialize::<T>(&mut payload)
                    .or(Err(anyhow!("Packets didn't contain a valid message")))?;
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
