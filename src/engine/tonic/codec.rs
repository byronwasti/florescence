use bytes::{Buf, BufMut};
use std::marker::PhantomData;
use tonic::{
    Status,
    codec::{Codec, DecodeBuf, Decoder, EncodeBuf, Encoder},
};

#[derive(Debug)]
pub struct BinCoder<T>(PhantomData<T>);

impl<T: serde::Serialize> Encoder for BinCoder<T> {
    type Item = T;
    type Error = Status;

    fn encode(&mut self, item: Self::Item, buf: &mut EncodeBuf<'_>) -> Result<(), Self::Error> {
        let _ = bincode::serde::encode_into_std_write(
            &item,
            &mut buf.writer(),
            bincode::config::standard(),
        )
        .map_err(|e| Status::internal(e.to_string()))?;
        Ok(())
    }
}

impl<U: serde::de::DeserializeOwned> Decoder for BinCoder<U> {
    type Item = U;
    type Error = Status;

    fn decode(&mut self, buf: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Self::Error> {
        if !buf.has_remaining() {
            return Ok(None);
        }

        let item: Self::Item =
            bincode::serde::decode_from_std_read(&mut buf.reader(), bincode::config::standard())
                .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Some(item))
    }
}

/// A [`Codec`] that implements `application/grpc+json` via the serde library.
#[derive(Debug, Clone)]
pub struct BinCodec<T, U>(PhantomData<(T, U)>);

impl<T, U> Default for BinCodec<T, U> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T, U> Codec for BinCodec<T, U>
where
    T: serde::Serialize + Send + 'static,
    U: serde::de::DeserializeOwned + Send + 'static,
{
    type Encode = T;
    type Decode = U;
    type Encoder = BinCoder<T>;
    type Decoder = BinCoder<U>;

    fn encoder(&mut self) -> Self::Encoder {
        BinCoder(PhantomData)
    }

    fn decoder(&mut self) -> Self::Decoder {
        BinCoder(PhantomData)
    }
}
