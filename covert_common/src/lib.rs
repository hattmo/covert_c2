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

use aes::Aes256;
use bincode::{
    config::{RejectTrailing, VarintEncoding, WithOtherIntEncoding, WithOtherTrailing},
    DefaultOptions, Options,
};
use cipher::{
    block_padding::Pkcs7, generic_array::GenericArray, BlockDecrypt, BlockEncrypt,
    KeyInit,
};

use rand::random;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
#[derive(Deserialize, Serialize, Debug)]
struct CovertPacket {
    hash: u8,
    stream: u16,
    syn: u32,
    want: u32,
    last: bool,
    payload: Vec<u8>,
    pad: Vec<u8>,
} //16 bytes max

impl CovertPacket {
    fn new(stream: u16, syn: u32, last: bool, payload: &[u8]) -> Self {
        let pad = vec![0u8; 30 - payload.len()];
        let pad: Vec<u8> = pad.iter().map(|_| random()).collect();

        Self {
            hash: 0,
            stream,
            syn,
            want: 0,
            last,
            payload: payload.to_vec(),
            pad,
        }
    }
}

struct CovertStream<T> {
    out_bound_packet_cache: VecDeque<CovertPacket>,
    in_bound_packet_cache: VecDeque<CovertPacket>,
    message_cache: VecDeque<T>,
    out_count: u32,
    out_syn: u32,
    in_syn: u32,
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
    engine: Aes256,
    encoder: WithOtherTrailing<
        WithOtherIntEncoding<DefaultOptions, VarintEncoding>,
        RejectTrailing,
    >,
    streams: HashMap<u16, CovertStream<T>>,
}

impl<T> CovertChannel<T>
where
    T: DeserializeOwned + Serialize,
{
    /// Create a new covert channel with the provided 32 byte aes key.
    pub fn new(key: [u8; 32]) -> CovertChannel<T> {
        let encoder = bincode::DefaultOptions::new()
            .with_varint_encoding()
            .reject_trailing_bytes();
        // let cryptor = with_key(create_key(key).expect("Bad key"));
        CovertChannel {
            encoder,
            engine: Aes256::new(&GenericArray::from(key)),
            streams: HashMap::new(),
        }
    }

    /// get a complete message T from this channel.  if there are no messages sent completely
    /// yet then the Option is None.
    pub fn get_message(&mut self, stream_id: u16) -> Option<T> {
        let stream = self.streams.get_mut(&stream_id)?;
        stream.message_cache.pop_front()
    }

    /// send a message through this channel.  
    pub fn put_message(&mut self, msg: T, stream_id: u16) -> () {
        let encode = self.encoder;
        let stream = self.get_stream_by_id(stream_id);
        let res = encode.serialize(&msg).expect("Failed to serialize");
        let (parts, end) = res.as_chunks::<29>();
        for part in parts {
            let new_packet =
                CovertPacket::new(stream_id, stream.out_count, false, part.as_slice());
            stream.out_count += 1;
            stream.out_bound_packet_cache.push_back(new_packet);
        }
        if end.len() != 0 {
            let last_packet = CovertPacket::new(stream_id, stream.out_count, true, end);
            stream.out_bound_packet_cache.push_back(last_packet);
            stream.out_count += 1;
        } else {
            stream.out_bound_packet_cache.back_mut().unwrap().last = true;
        }
    }

    /// get the next packet that needs to be sent for this channel.  Even if there are
    /// no messages to be sent a call to get_packet with return successfully.  empty packets
    /// still contain synchronizing information and should be sent regularly.  additionally
    /// the receiving end of the channel accounts for packets that contain no message data.
    pub fn get_packet(&mut self, stream_id: u16) -> Vec<u8> {
        let encode = self.encoder;
        let stream = self.get_stream_by_id(stream_id);
        if stream.out_bound_packet_cache.len() == 0 {
            stream.out_bound_packet_cache.push_front(CovertPacket::new(
                stream_id,
                stream.out_count,
                false,
                &[],
            ));
            stream.out_count += 1;
        }
        let out = stream
            .out_bound_packet_cache
            .front_mut()
            .expect("Should always have a packet");
        out.want = stream.in_syn;
        let mut tmp = encode.serialize(out).expect("Should always serialize");
        let hash = crc32fast::hash(&tmp).to_le_bytes()[0];
        tmp[0] = hash;
        let done = self.engine.encrypt_padded_vec::<Pkcs7>(&tmp);
        return done;
    }

    /// put packets into this channel to be decoded.  When a complete packet is ready to
    /// be read from the channel put packet will contain true in the Ok result.
    pub fn put_packet(&mut self, pkt: &[u8]) -> Option<u16> {
        let encode = self.encoder;
        let mut tmp = self.engine.decrypt_padded_vec::<Pkcs7>(pkt).ok()?;
        let hash = tmp[0];
        tmp[0] = 0;
        let actual = crc32fast::hash(&tmp).to_le_bytes()[0];
        if hash != actual {
            return None;
        };
        let in_packet = encode.deserialize::<CovertPacket>(&tmp).ok()?;

        let stream_id = in_packet.stream;
        let mut stream = self.get_stream_by_id(stream_id);

        // Clear packets in the out_cache that have been confirmed
        stream.out_syn = Ord::max(stream.out_syn, in_packet.want);
        stream
            .out_bound_packet_cache
            .retain(|item| item.syn >= stream.out_syn);

        // Append to the in_cache if this is the next packet needed else drop it
        if in_packet.syn == stream.in_syn {
            stream.in_syn += 1;
            let is_last = in_packet.last;
            stream.in_bound_packet_cache.push_back(in_packet);
            if is_last {
                let mut payload: Vec<u8> = stream
                    .in_bound_packet_cache
                    .drain(..)
                    .map(|i| i.payload)
                    .flatten()
                    .collect();
                let in_message = encode.deserialize::<T>(&mut payload).ok()?;
                stream.message_cache.push_back(in_message);
                return Some(stream_id);
            } else {
                return None;
            }
        }
        return None;
    }

    fn get_stream_by_id(&mut self, stream_id: u16) -> &mut CovertStream<T> {
        if !self.streams.contains_key(&stream_id) {
            let new_stream = CovertStream {
                out_bound_packet_cache: VecDeque::new(),
                in_bound_packet_cache: VecDeque::new(),
                message_cache: VecDeque::new(),
                out_count: 0,
                out_syn: 0,
                in_syn: 0,
            };
            self.streams.insert(stream_id, new_stream);
        }
        self.streams.get_mut(&stream_id).unwrap()
    }
}

#[cfg(test)]
mod test;
