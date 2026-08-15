use eventsource_stream::{EventStreamError, Eventsource};
use futures_util::{Stream, StreamExt};

#[derive(Debug)]
pub(super) struct Event {
    pub data: String,
}

#[derive(Debug)]
pub(super) enum Error<E> {
    InvalidData(String),
    Transport(E),
}

pub(super) fn events<S, B, E>(stream: S) -> impl Stream<Item = Result<Event, Error<E>>>
where
    S: Stream<Item = Result<B, E>>,
    B: AsRef<[u8]>,
{
    stream.eventsource().map(|result| match result {
        Ok(event) => Ok(Event { data: event.data }),
        Err(EventStreamError::Utf8(error)) => {
            Err(Error::InvalidData(format!("invalid UTF-8: {error}")))
        }
        Err(EventStreamError::Parser(error)) => {
            Err(Error::InvalidData(format!("invalid SSE data: {error:?}")))
        }
        Err(EventStreamError::Transport(error)) => Err(Error::Transport(error)),
    })
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use futures_util::{stream, StreamExt};

    use super::events;

    #[test]
    fn parses_fragmented_utf8_and_multiple_events() {
        let body = "data: 你好\n\nevent: usage\r\ndata: {\"total\":\r\ndata: 3}\r\n\r\n";
        let bytes = body.as_bytes();
        let stream = stream::iter([
            Ok::<_, Infallible>(&bytes[..7]),
            Ok(&bytes[7..17]),
            Ok(&bytes[17..]),
        ]);

        let parsed = tauri::async_runtime::block_on(events(stream).collect::<Vec<_>>());

        assert_eq!(parsed.len(), 2);

        let text = parsed[0].as_ref().expect("valid text event");
        assert_eq!(text.data, "你好");

        let usage = parsed[1].as_ref().expect("valid usage event");
        assert_eq!(usage.data, "{\"total\":\n3}");
    }
}
