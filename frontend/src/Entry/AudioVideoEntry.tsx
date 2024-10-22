import { useContext, useEffect, useRef, useState } from "react";
import { useFormContext, useFieldArray } from "react-hook-form";

import { uuidv4 } from "../uuid";
import { getUserMedia } from "../media";
import { EntryForm } from "../journal";

interface RecordAudioVideoProps {
    on_cancel: () => void,
    on_created: (URL, Blob) => void,
}

function RecordAudioVideo({on_cancel, on_created}: RecordAudioVideoProps) {
    const preview_ele_ref = useRef<HTMLMediaElement>(null);
    const media_ref = useRef<{
        stream: MediaStream,
        recorder: MediaRecorder,
        buffer: Blob[],
        blob: Blob
    }>({
        stream: null,
        recorder: null,
        buffer: [],
        blob: null,
    });

    let [msg, setMsg] = useState(" ");
    let [recording_ready, setRecordingReady] = useState(false);
    let [recording_started, setRecordingStarted] = useState(false);
    let [recording_paused, setRecordingPaused] = useState(false);

    const create_media_recorder = (): Promise<MediaRecorder> => {
        return getUserMedia({audio: true, video: true}).then((result) => {
            if (result == null) {
                setMsg("failed to get audio stream for recording");

                return null;
            }

            const media_recorder = new MediaRecorder(result, {mimeType: "video/webm"});
            media_ref.current.stream = result;
            media_ref.current.recorder = media_recorder;

            preview_ele_ref.current.srcObject = result;

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

                on_created(
                    URL.createObjectURL(media_ref.current.blob),
                    media_ref.current.blob
                );
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
    };

    const stop_streams = () => {
        if (media_ref.current.stream != null) {
            media_ref.current.stream.getTracks().forEach(track => {
                if (track.readyState === "live") {
                    track.stop();
                }
            });
        }
    };

    const start_recording = () => {
        if (media_ref.current.recorder != null) {
            media_ref.current.recorder.start(1000);
        }
    };

    const stop_recording = () => {
        if (media_ref.current.recorder != null) {
            if (media_ref.current.recorder.state === "recording" ||
                media_ref.current.recorder.state === "paused"
            ) {
                media_ref.current.recorder.stop();
            }
        }
    };

    const resume_recording = () => {
        if (media_ref.current.recorder != null) {
            if (media_ref.current.recorder.state === "paused") {
                media_ref.current.recorder.resume();
            }
        }
    };

    const pause_recording = () => {
        if (media_ref.current.recorder != null) {
            if (media_ref.current.recorder.state === "recording") {
                media_ref.current.recorder.pause();
            }
        }
    };

    useEffect(() => {
        return () => stop_streams();
    }, []);

    useEffect(() => {
        create_media_recorder();
    }, []);

    return <div>
        <button
            type="button"
            disabled={!recording_ready}
            onClick={() => {
                on_cancel();
            }}
        >
            Cancel
        </button>
        <button
            type="button"
            disabled={!recording_ready}
            onClick={() => {
                if (recording_started) {
                    stop_recording();
                } else {
                    start_recording();
                }
            }}
        >
            {recording_ready && recording_started ? "Stop" : "Start"}
        </button>
        <button
            type="button"
            disabled={!recording_started}
            onClick={() => {
                if (recording_paused) {
                    resume_recording();
                } else {
                    pause_recording();
                }
            }}
        >
            {recording_ready && recording_paused ? "Resume" : "Pause"}
        </button>
        <video
            ref={preview_ele_ref}
            id="preview"
            width="160"
            height="120"
            autoPlay
            muted
            onPlaying={() => {
                console.log("video is playing");

                setRecordingReady(true);
            }}
        />
    </div>
}

interface AudioVideoProps {}

function AudioVideo({}: AudioVideoProps) {
    const [record_video, set_record_video] = useState(false);

    const form = useFormContext<EntryForm>();
    const video = useFieldArray<EntryForm, "video">({
        control: form.control,
        name: "video"
    });

    return <div>
        <div>
            {record_video ?
                <RecordAudioVideo
                    on_cancel={() => {
                        set_record_video(false);
                    }}
                    on_created={(url, blob) => {
                        video.append({
                            type: "in-memory",
                            src: url,
                            data: blob
                        });

                        set_record_video(false);
                    }}
                />
                :
                <button
                    type="button"
                    onClick={() => {
                        set_record_video(true);
                    }}
                >
                    Add
                </button>
            }
        </div>
        <div>
            {video.fields.map((field, index) => {
                console.log("video field:", field);

                return <video
                    key={field.id}
                    src={field.src}
                    width="160"
                    height="120"
                    controls
                />
            })}
        </div>
    </div>
}

export default AudioVideo;
