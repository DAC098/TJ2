import { useContext, useEffect, useRef, useState } from "react"
import { useFormContext, useFieldArray } from "react-hook-form";

import { uuidv4 } from "../uuid";
import { getUserMedia } from "../media";
import { EntryForm } from "../journal";

interface RecordAudioProps {
    on_cancel: () => void,
    on_created: (URL) => void,
}

function RecordAudio({on_cancel, on_created}: RecordAudioProps) {
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

                stop_streams();

                media_ref.current.buffer = [];

                on_created(URL.createObjectURL(media_ref.current.blob));
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
        <button type="button" onClick={() => {
            on_cancel();
        }}>Cancel</button>
        <button type="button" onClick={() => {
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
}

interface AudioEntryProps {}

const AudioEntry = ({}: AudioEntryProps) => {
    const [record_audio, set_record_audio] = useState(false);

    const form = useFormContext<EntryForm>();
    const audio = useFieldArray<EntryForm, "audio">({
        control: form.control,
        name: "audio"
    });

    return <div>
        <div>
            {record_audio ?
                <RecordAudio on_cancel={() => {
                    set_record_audio(false);
                }} on_created={url => {
                    audio.append({src: url});

                    set_record_audio(false);
                }}/>
                :
                <button type="button" onClick={() => {
                    set_record_audio(true);
                }}>
                    Add
                </button>
            }
        </div>
        <div>
            {audio.fields.map((field, index) => {
                console.log("audio field:", field);

                return <audio key={field.id} src={field.src} controls/>
            })}
        </div>
    </div>
}

export default AudioEntry;
