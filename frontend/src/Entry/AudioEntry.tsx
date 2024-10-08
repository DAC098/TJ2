import { useContext, useEffect, useRef, useState } from "react"
import { getUserMedia } from "../media";

interface AudioEntryProps {}

const AudioEntry = ({}: AudioEntryProps) => {
    let audio_ele_ref = useRef<HTMLAudioElement>(null);
    let media_ref = useRef<{
        stream: MediaStream,
        recorder: MediaRecorder,
        buffer: Blob[],
        blob: Blob
    }>({
        stream: null,
        recorder: null,
        buffer: [],
        blob: null
    });
    let [msg, setMsg] = useState(" ");
    let [recording_started, setRecordingStarted] = useState(false);
    let [recording_paused, setRecordingPaused] = useState(false);
    let [recording_finished, setRecordingFinished] = useState(false);

    const stop_streams = () => {
        if (media_ref.current.stream != null) {
            media_ref.current.stream.getTracks().forEach(track => {
                if (track.readyState === "live") {
                    track.stop();
                }
            });
        }
    }

    const create_media_recorder = (): Promise<MediaRecorder> => {
        return getUserMedia({audio: true}).then((result) => {
            if (result == null) {
                setMsg("failed to get audio stream for recording");

                return null;
            }

            media_ref.current.stream = result;
            const media_recorder = new MediaRecorder(result, {mimeType: "audio/webm"});
            media_ref.current.recorder = media_recorder;

            media_recorder.addEventListener("dataavailable", (e) => {
                if (e.data.size > 0) {
                    media_ref.current.buffer.push(e.data);
                }
            });

            media_recorder.addEventListener("stop", (e) => {
                setRecordingStarted(false);

                if (media_ref.current.blob != null) {
                    media_ref.current.blob = new Blob(
                        [
                            media_ref.current.blob,
                            ...media_ref.current.buffer
                        ]
                    );
                } else {
                    media_ref.current.blob = new Blob(media_ref.current.buffer);
                }

                media_ref.current.buffer = [];

                if (audio_ele_ref.current != null) {
                    audio_ele_ref.current.src = URL.createObjectURL(
                        media_ref.current.blob
                    );
                }

                stop_streams();
                setRecordingFinished(true);
            });

            media_recorder.addEventListener("pause", (e) => {
                setRecordingPaused(true);
            });

            media_recorder.addEventListener("start", (e) => {
                setRecordingStarted(true);
            });

            media_recorder.addEventListener("resume", (e) => {
                setRecordingPaused(false);
            });

            media_recorder.addEventListener("error", (e) => {
                console.log("media recorder error", e);
            });

            return media_recorder;
        }).catch(err => {
            if (err.name === "AbortError") {
                setMsg("something caused an error. aborting");
            } else if (err.name === "NotAllowedError") {
                setMsg("the site is not allowed to access your microphone");
            } else if (err.name === "NotFoundError") {
                setMsg("no audio recording device was found for the system");
            } else {
                setMsg("unhandled error: " + err.name);
            }

            return null;
        })
    }

    const start_recording = () => {
        if (media_ref.current.recorder == null) {
            create_media_recorder().then(recorder => {
                recorder?.start(1000);
            });
        } else {
            media_ref.current.recorder.start(1000);
        }
    }

    const stop_recording = () => {
        if (media_ref.current.recorder != null) {
            if (media_ref.current.recorder.state === "recording" ||
                media_ref.current.recorder.state === "paused"
            ) {
                media_ref.current.recorder.stop();
            }
        }
    }

    const pause_recording = () => {
        if (media_ref.current.recorder != null) {
            if (media_ref.current.recorder.state === "recording") {
                media_ref.current.recorder.pause();
            }
        }
    }

    const resume_recording = () => {
        if (media_ref.current.recorder != null) {
            if (media_ref.current.recorder.state === "paused") {
                media_ref.current.recorder.resume();
            }
        }
    }

    useEffect(() => {
        return () => stop_streams();
    }, []);

    return <div>
        <div>
            {!recording_finished ?
                <div>
                    <button type="button" disabled={false} onClick={() => {
                        if (false) {
                            return;
                        }

                        if (recording_started) {
                            stop_recording();
                        } else {
                            start_recording();
                        }
                    }}>{recording_started ? "Stop" : "Start"}</button>
                    <button type="button" disabled={!recording_started} onClick={() => {
                        if (recording_paused) {
                            resume_recording();
                        } else {
                            pause_recording();
                        }
                    }}>{recording_paused ? "Resume" : "Pause"}</button>
                </div>
                :
                <audio ref={audio_ele_ref} controls/>
            }
        </div>
        <span>{msg}</span>
    </div>
}

export default AudioEntry;
