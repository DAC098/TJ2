use std::pin::Pin;

use bytes::{Bytes, BytesMut};
use futures::{
    task::{self, Poll},
    Stream,
};
use pin_project::pin_project;
use ringbuf::traits::{Consumer, Observer, Producer};

/// read the total amount of bytes passed through the stream from the given
/// provider stream
///
/// currently only supports streams where the item is a [`Result`] with [`Ok`]
/// values that can be referenced into a [`u8`] slice.
#[pin_project]
pub struct BytesRead<'a, S: Stream> {
    counter: &'a mut usize,

    #[pin]
    producer: S,
}

impl<'a, S> BytesRead<'a, S>
where
    S: Stream,
{
    /// creates a new [`BytesRead`] for the given producer stream and counter
    #[allow(dead_code)]
    pub fn new(producer: S, counter: &'a mut usize) -> Self {
        Self { counter, producer }
    }
}

impl<'a, S, I, E> Stream for BytesRead<'a, S>
where
    I: AsRef<[u8]>,
    S: Stream<Item = Result<I, E>>,
{
    type Item = Result<I, E>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        match this.producer.poll_next(cx) {
            Poll::Ready(Some(maybe)) => match maybe {
                Ok(slice) => {
                    **this.counter += slice.as_ref().len();

                    Poll::Ready(Some(Ok(slice)))
                }
                Err(err) => Poll::Ready(Some(Err(err))),
            },
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// error indicating if the [`MaxBytes`] stream has exceeded is max amount of
/// bytes or encountered an error from the underlying stream.
pub enum MaxBytesError<E> {
    MaxSize,
    Producer(E),
}

impl<E> std::fmt::Debug for MaxBytesError<E>
where
    E: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MaxSize => f.write_str("MaxSize"),
            Self::Producer(e) => e.fmt(f),
        }
    }
}

impl<E> std::fmt::Display for MaxBytesError<E>
where
    E: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MaxSize => f.write_str("exceeded maximum amount of bytes"),
            Self::Producer(e) => e.fmt(f),
        }
    }
}

impl<E> std::error::Error for MaxBytesError<E>
where
    E: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Producer(e) => Some(e),
            _ => None,
        }
    }
}

/// limits a stream from going over a certain byte threshold
///
/// if the total bytes read exceeds the specified max then it will throw a
/// [`MaxBytesError`]
pub struct MaxBytes {
    total: usize,
    max: usize,
}

#[pin_project]
pub struct MaxBytesStream<'a, S: Stream> {
    parent: &'a mut MaxBytes,

    #[pin]
    producer: S,
}

impl MaxBytes {
    /// creates a new [`MaxBytes`] for the given max bytes
    pub fn new(max: usize) -> Self {
        Self { total: 0, max }
    }

    pub fn get_total(&self) -> usize {
        self.total
    }

    pub fn for_stream<S, I, E>(&mut self, producer: S) -> MaxBytesStream<'_, S>
    where
        I: AsRef<[u8]>,
        S: Stream<Item = Result<I, E>>,
    {
        MaxBytesStream {
            parent: self,
            producer,
        }
    }
}

impl<'a, S, I, E> Stream for MaxBytesStream<'a, S>
where
    I: AsRef<[u8]>,
    S: Stream<Item = Result<I, E>>,
{
    type Item = Result<I, MaxBytesError<E>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        match this.producer.poll_next(cx) {
            Poll::Ready(Some(maybe)) => match maybe {
                Ok(slice) => {
                    let len = slice.as_ref().len();

                    // the amount of data that we would need to handle some how
                    // exceeds a usize which depending on the system could be
                    // larger than a 64bit unsigned integer
                    if let Some(valid) = this.parent.total.checked_add(len) {
                        if valid > this.parent.max {
                            tracing::trace!(
                                "reached max bytes. total: {valid} > {}",
                                this.parent.max
                            );

                            Poll::Ready(Some(Err(MaxBytesError::MaxSize)))
                        } else {
                            tracing::trace!("updating total: {valid}");

                            this.parent.total = valid;

                            Poll::Ready(Some(Ok(slice)))
                        }
                    } else {
                        Poll::Ready(Some(Err(MaxBytesError::MaxSize)))
                    }
                }
                Err(err) => Poll::Ready(Some(Err(MaxBytesError::Producer(err)))),
            },
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// updates a [`blake3::Hasher`] with the contents of a given stream
#[pin_project]
pub struct HashStream<'a, S: Stream> {
    hasher: &'a mut blake3::Hasher,

    #[pin]
    producer: S,
}

impl<'a, S> HashStream<'a, S>
where
    S: Stream,
{
    /// creates a new [`HashStream`] for the given producer stream and
    /// [`blake3::Hasher`]
    pub fn new(producer: S, hasher: &'a mut blake3::Hasher) -> Self {
        Self { hasher, producer }
    }
}

impl<'a, S, I, E> Stream for HashStream<'a, S>
where
    I: AsRef<[u8]>,
    S: Stream<Item = Result<I, E>>,
{
    type Item = Result<I, E>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        match this.producer.poll_next(cx) {
            Poll::Ready(Some(maybe)) => match maybe {
                Ok(slice) => {
                    this.hasher.update(slice.as_ref());

                    Poll::Ready(Some(Ok(slice)))
                }
                Err(err) => Poll::Ready(Some(Err(err))),
            },
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// attempts to capture the last `N` amount of bytes from a stream
///
/// to be used in conjunction with [`CaptureTrailingStream`]
pub struct CaptureTrailing {
    /// total amount of bytes to capture
    capture: usize,
    /// amount of additional buffer storage
    size: usize,
    /// ring buffer to use for storing bytes
    buffer: ringbuf::HeapRb<u8>,
}

/// stream interface for updating a [`CaptureTrailing`] struct
#[pin_project]
pub struct CaptureTrailingStream<'a, S, I, E>
where
    I: AsRef<[u8]>,
    S: Stream<Item = Result<I, E>>,
{
    parent: &'a mut CaptureTrailing,
    current: Option<(I, usize)>,
    finished: bool,

    #[pin]
    producer: S,
}

impl CaptureTrailing {
    /// creates a new [`CaptureTrailing`] for the given capture size
    ///
    /// creates an additional buffer size of `8192` bytes
    pub fn new(capture: usize) -> Self {
        Self::with_size(1024 * 8, capture)
    }

    /// creates a new [`CaptureTrailing`] for the given buffer size and capture
    /// size
    pub fn with_size(size: usize, capture: usize) -> Self {
        let buffer = ringbuf::HeapRb::new(size + capture);

        Self {
            capture,
            size,
            buffer,
        }
    }

    /// pops out the current contents of the ring buffer into a [`Vec`] of
    /// [`u8`]'s
    pub fn pop_occupied(&mut self) -> Vec<u8> {
        let mut rtn = vec![0u8; self.buffer.occupied_len()];

        self.buffer.pop_slice(&mut rtn);

        rtn
    }

    /// creates a [`CaptureTrailingStream`] for the given stream and holds a
    /// reference to current [`CaptureTrailing`] struct
    pub fn for_stream<S, I, E>(&mut self, producer: S) -> CaptureTrailingStream<'_, S, I, E>
    where
        I: AsRef<[u8]>,
        S: Stream<Item = Result<I, E>>,
    {
        CaptureTrailingStream {
            parent: self,
            current: None,
            finished: false,
            producer,
        }
    }
}

impl<'a, S, I, E> Stream for CaptureTrailingStream<'a, S, I, E>
where
    I: AsRef<[u8]>,
    S: Stream<Item = Result<I, E>>,
{
    type Item = Result<Bytes, E>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        if let Some((curr, offset)) = this.current.take() {
            let slice_ref = curr.as_ref();
            let sub_slice = &slice_ref[offset..];

            let pushed = this.parent.buffer.push_slice(sub_slice);

            if pushed == sub_slice.len() {
                // we have finished off the current set of bytes. try to get
                // more by polling the producer
                tracing::trace!("pushed {pushed} bytes to buffer. continuing to producer");
            } else {
                // we still have more bytes to remove
                let mut to_pass = BytesMut::zeroed(this.parent.size);
                let popped = this.parent.buffer.pop_slice(&mut to_pass);

                *this.current = Some((curr, offset + pushed));

                tracing::trace!("stored bytes. pushed {pushed} bytes and popped {popped}");

                return Poll::Ready(Some(Ok(to_pass.freeze())));
            }
        }

        if *this.finished {
            return Poll::Ready(None);
        }

        loop {
            // poll the producer for more data. we have a chance to poll more
            // than once so we will need to take the producer with as_mut so we
            // can use the pin again
            match this.producer.as_mut().poll_next(cx) {
                Poll::Ready(Some(maybe)) => match maybe {
                    Ok(slice) => {
                        let slice_ref = slice.as_ref();

                        let pushed = this.parent.buffer.push_slice(slice_ref);

                        // were able to push the entire contents into the ring
                        // buffer loop back to poll the producer again for more
                        // data
                        if pushed == slice_ref.len() {
                            tracing::trace!(
                                "pushed {pushed} bytes to buffer. waiting for more from producer"
                            );

                            continue;
                        }

                        // since we will only pull off what we dont want to
                        // capture we dont have to worry about taking more than
                        // we are supposed
                        let mut to_pass = BytesMut::zeroed(this.parent.size);
                        let popped = this.parent.buffer.pop_slice(&mut to_pass);

                        *this.current = Some((slice, pushed));

                        tracing::trace!("pushed {pushed} bytes and popped {popped}. continue processing current bytes");

                        return Poll::Ready(Some(Ok(to_pass.freeze())));
                    }
                    Err(err) => return Poll::Ready(Some(Err(err))),
                },
                Poll::Ready(None) => {
                    // we may still have more data in the buffer than we need so
                    // we will need to check before sending Poll::Ready(None)
                    *this.finished = true;

                    let occupied = this.parent.buffer.occupied_len();

                    if occupied > this.parent.capture {
                        // still left over data that we will need to deal with
                        let mut to_pass = BytesMut::zeroed(occupied - this.parent.capture);

                        let popped = this.parent.buffer.pop_slice(&mut to_pass);

                        tracing::trace!(
                            "provider stream finished. occupied: {occupied} popped: {popped}"
                        );

                        return Poll::Ready(Some(Ok(to_pass.freeze())));
                    } else {
                        // we are done and the amount of data that we have is
                        // not enough so bail out
                        tracing::trace!("provider stream finished.");

                        return Poll::Ready(None);
                    }
                }
                Poll::Pending => {
                    tracing::trace!("provider stream pending");

                    return Poll::Pending;
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use futures::{AsyncWriteExt, SinkExt, StreamExt};

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn bytes_read() {
        let test_stream = futures::stream::iter(vec![
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
        ]);
        let mut written = 0usize;

        BytesRead::new(test_stream, &mut written)
            .forward(futures::io::sink().into_sink())
            .await
            .unwrap();

        assert_eq!(written, 50)
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn hash_stream() {
        let test_stream = futures::stream::iter(vec![
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
        ]);
        let mut hasher = blake3::Hasher::new();

        HashStream::new(test_stream, &mut hasher)
            .forward(futures::io::sink().into_sink())
            .await
            .unwrap();

        let hash = hasher.finalize();
        let expected = blake3::Hash::from_bytes([
            0x9d, 0x22, 0xa1, 0xf9, 0x2a, 0x85, 0xe4, 0x07, 0xb9, 0xfb, 0x4a, 0x3d, 0x69, 0xbb,
            0xdf, 0xcc, 0xc6, 0xad, 0xb9, 0x45, 0x01, 0x69, 0xae, 0xbd, 0x4d, 0x7e, 0x7b, 0xcc,
            0x20, 0xe7, 0x98, 0xdf,
        ]);

        assert_eq!(expected, hash);
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn max_bytes() {
        let test_vec: Vec<Result<Vec<u8>, std::io::Error>> = vec![
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
        ];
        let test_stream = futures::stream::iter(test_vec);
        let mut max_bytes = MaxBytes::new(50);

        let result = max_bytes
            .for_stream(test_stream)
            .forward(
                futures::io::sink()
                    .into_sink()
                    .sink_map_err(|e| MaxBytesError::Producer(e)),
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn max_bytes_fail() {
        let test_vec: Vec<Result<Vec<u8>, std::io::Error>> = vec![
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
        ];
        let test_stream = futures::stream::iter(test_vec);
        let mut max_bytes = MaxBytes::new(40);

        let result = max_bytes
            .for_stream(test_stream)
            .forward(
                futures::io::sink()
                    .into_sink()
                    .sink_map_err(|e| MaxBytesError::Producer(e)),
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn capture_trailing() {
        let test_vec: Vec<Result<Vec<u8>, std::io::Error>> = vec![
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![9, 8, 7, 6, 5, 4, 3, 2, 1, 0]),
        ];
        let test_stream = futures::stream::iter(test_vec);
        let mut capture = CaptureTrailing::with_size(10, 10);

        capture
            .for_stream(test_stream)
            .forward(futures::io::sink().into_sink())
            .await
            .unwrap();

        let remaining = capture.pop_occupied();

        assert_eq!(remaining, vec![9, 8, 7, 6, 5, 4, 3, 2, 1, 0]);
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn capture_trailing_large_buffer() {
        let test_vec: Vec<Result<Vec<u8>, std::io::Error>> = vec![
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![9, 8, 7, 6, 5, 4, 3, 2, 1, 0]),
        ];
        let test_stream = futures::stream::iter(test_vec);
        let mut capture = CaptureTrailing::with_size(25, 10);

        capture
            .for_stream(test_stream)
            .forward(futures::io::sink().into_sink())
            .await
            .unwrap();

        let remaining = capture.pop_occupied();

        assert_eq!(remaining, vec![9, 8, 7, 6, 5, 4, 3, 2, 1, 0]);
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn capture_trailing_small_buffer() {
        let test_vec: Vec<Result<Vec<u8>, std::io::Error>> = vec![
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            Ok(vec![9, 8, 7, 6, 5, 4, 3, 2, 1, 0]),
        ];
        let test_stream = futures::stream::iter(test_vec);
        let mut capture = CaptureTrailing::with_size(5, 10);

        capture
            .for_stream(test_stream)
            .forward(futures::io::sink().into_sink())
            .await
            .unwrap();

        let remaining = capture.pop_occupied();

        assert_eq!(remaining, vec![9, 8, 7, 6, 5, 4, 3, 2, 1, 0]);
    }
}
