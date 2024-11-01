
import { useContext, useEffect, useRef, useState } from "react"
import { useFormContext, useFieldArray } from "react-hook-form";

import { uuidv4 } from "../uuid";
import { getUserMedia } from "../media";
import { EntryForm, timestamp_name } from "../journal";

interface RecordAudioProps {
    on_cancel: () => void,
    on_created: (URL, Blob) => void,
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

                let blob_options = {
                    type: "audio/webm"
                };

                if (media_ref.current.blob != null) {
                    media_ref.current.blob = new Blob(
                        [
                            media_ref.current.blob,
                            ...media_ref.current.buffer
                        ],
                        blob_options
                    );
                } else {
                    media_ref.current.blob = new Blob(
                        media_ref.current.buffer,
                        blob_options
                    );
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

interface RecordVideoProps {
    on_cancel: () => void,
    on_created: (URL, Blob) => void,
}

function RecordVideo({on_cancel, on_created}: RecordVideoProps) {
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

                let blob_options = {
                    type: "video/webm"
                };

                if (media_ref.current.blob != null) {
                    media_ref.current.blob = new Blob(
                        [
                            media_ref.current.blob,
                            ...media_ref.current.buffer
                        ],
                        blob_options
                    );
                } else {
                    media_ref.current.blob = new Blob(
                        media_ref.current.buffer,
                        blob_options
                    );
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

enum FileOption {
    None = 0,
    RecAudio = 1,
    RecVideo = 2,
}

interface FileEntryProps {
    entry_date: string,
}

const FileEntry = ({entry_date}: FileEntryProps) => {
    const [file_option, set_file_option] = useState<FileOption>(FileOption.None);

    const form = useFormContext<EntryForm>();
    const files = useFieldArray<EntryForm, "files">({
        control: form.control,
        name: "files"
    });

    let file_ele;

    switch (file_option) {
    case FileOption.None:
        file_ele = <div>
            <button type="button" onClick={() => {
                set_file_option(FileOption.RecAudio);
            }}>
                Rec Audio
            </button>
            <button type="button" onClick={() => {
                set_file_option(FileOption.RecVideo);
            }}>
                Rec Video
            </button>
        </div>;
        break;
    case FileOption.RecAudio:
        file_ele = <RecordAudio
            on_cancel={() => {
                set_file_option(FileOption.None);
            }}
            on_created={(url, blob) => {
                let date = new Date();

                files.append({
                    type: "in-memory",
                    key: uuidv4(),
                    src: url,
                    data: blob,
                    name: `${timestamp_name()}_audio`,
                    mime_type: "audio",
                    mime_subtype: "webm",
                    mime_param: null,
                });

                set_file_option(FileOption.None);
            }}
        />;
        break;
    case FileOption.RecVideo:
        file_ele = <RecordVideo
            on_cancel={() => {
                set_file_option(FileOption.None);
            }}
            on_created={(url, blob) => {
                let date = new Date();

                files.append({
                    type: "in-memory",
                    key: uuidv4(),
                    src: url,
                    data: blob,
                    name: `${timestamp_name()}_video`,
                    mime_type: "video",
                    mime_subtype: "webm",
                    mime_param: null,
                });

                set_file_option(FileOption.None);
            }}
        />;
        break;
    }

    return <div>
        <div>{file_ele}</div>
        <div>
            {files.fields.map((field, index) => {
                console.log("files field:", field);

                let src;

                switch (field.type) {
                case "server":
                    src = `/entries/${entry_date}/${field._id}`;
                    break;
                default:
                    src = field.src;
                    break;
                }

                switch (field.mime_type) {
                case "audio":
                    return <div key={field.id}>
                        <input type="text" {...form.register(`files.${index}.name`)}/>
                        <audio  src={src} controls/>
                    </div>;
                case "video":
                    return <div key={field.id}>
                        <input type="text" {...form.register(`files.${index}.name`)}/>
                        <video
                            src={src}
                            width="160"
                            height="120"
                            controls
                        />
                    </div>
                default:
                    return <div key={field.id}>
                        <input type="text" {...form.register(`files.${index}.name`)}/>
                        <div>file: {field.type}</div>
                    </div>
                }
            })}
        </div>
    </div>
}

export default FileEntry;
