import { useContext, useEffect, useRef, useState } from "react"
import { Video, CirclePause, CirclePlay, SquareArrowOutUpRight } from "lucide-react";
import { Root as VisuallyHidden } from "@radix-ui/react-visually-hidden";

import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogTitle,
    DialogTrigger,
} from "@/components/ui/dialog";
import { getUserMedia } from "@/media";
import { useObjectUrl } from "@/hooks";

export interface RecordVideoProps {
    on_created: (Blob) => void,
    disabled?: boolean
}

export function RecordVideo({on_created, disabled = false}: RecordVideoProps) {
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
    });

    let [dialog_open, set_dialog_open] = useState(false);
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

            media_ref.current.stream = result;

            const media_recorder = new MediaRecorder(result, {mimeType: "video/webm"});
            media_ref.current.recorder = media_recorder;

            preview_ele_ref.current.srcObject = result;

            media_recorder.addEventListener("dataavailable", (e) => {
                if (e.data.size > 0) {
                    media_ref.current.buffer.push(e.data);
                }
            });

            media_recorder.addEventListener("stop", (e) => {
                setRecordingStarted(false);

                let blob = new Blob(
                    media_ref.current.buffer,
                    {type: "video/webm"}
                );

                stop_streams();

                on_created(blob);

                media_ref.current.buffer = [];

                set_dialog_open(false);
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
                setMsg("the site is not allowed to access your microphone/camera");
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

    return <Dialog open={dialog_open} onOpenChange={(open) => {
        if (open) {
            create_media_recorder();
        } else {
            stop_streams();
        }

        set_dialog_open(open);
    }}>
        <DialogTrigger asChild>
            <Button type="button" variant="secondary" disabled={disabled}>Add Video<Video/></Button>
        </DialogTrigger>
        <VisuallyHidden>
            <DialogTitle>Record Video</DialogTitle>
            <DialogDescription>
                Records audio and video to be stored with a journal entry
            </DialogDescription>
        </VisuallyHidden>
        <DialogContent>
            <video
                ref={preview_ele_ref}
                id="preview"
                autoPlay
                muted
                onPlaying={() => {
                    console.log("video is playing");

                    setRecordingReady(true);
                }}
            />
            <div className="flex flex-row flex-nowrap items-center justify-center gap-x-4">
                <Button
                    type="button"
                    variant="outline"
                    disabled={!recording_ready}
                    onClick={() => {
                        if (recording_started) {
                            stop_recording();
                        } else {
                            start_recording();
                        }
                    }}
                >
                    {recording_started ? "Stop" : "Start"}
                </Button>
                <Button
                    type="button"
                    variant="outline"
                    disabled={!recording_ready || !recording_started}
                    onClick={() => {
                        if (recording_paused) {
                            resume_recording();
                        } else {
                            pause_recording();
                        }
                    }}
                >
                    {recording_paused ?
                        <>Resume <CirclePlay/></>
                        :
                        <>Pause <CirclePause/></>
                    }
                </Button>
            </div>
        </DialogContent>
    </Dialog>
}

export interface PlayVideoProps {
    src: string | File | Blob
}

export function PlayVideo({src}: PlayVideoProps) {
    let url = useObjectUrl(src);

    return <Dialog>
        <DialogTrigger asChild>
            <Button type="button" variant="secondary" size="icon">
                <SquareArrowOutUpRight/>
            </Button>
        </DialogTrigger>
        <VisuallyHidden>
            <DialogTitle>Play Video</DialogTitle>
            <DialogDescription>
                Plays audio video files attached to a journal entry
            </DialogDescription>
        </VisuallyHidden>
        <DialogContent>
            <video controls src={url}/>
        </DialogContent>
    </Dialog>;
}
