import { useContext, useEffect, useRef, useState } from "react"
import { Mic, CirclePause, CirclePlay, CircleStop, SquareArrowOutUpRight } from "lucide-react";
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

export interface RecordAudioProps {
    on_created: (Blob) => void,
    disabled?: boolean
}

export function RecordAudio({on_created, disabled}: RecordAudioProps) {
    let media_ref = useRef<{
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
    let [recording_started, setRecordingStarted] = useState(false);
    let [recording_paused, setRecordingPaused] = useState(false);

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

                let blob = new Blob(
                    media_ref.current.buffer,
                    {type: "audio/webm"}
                );

                stop_streams();

                media_ref.current.buffer = [];

                on_created(blob);

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

    const pause_recording = () => {
        if (media_ref.current.recorder != null) {
            if (media_ref.current.recorder.state === "recording") {
                media_ref.current.recorder.pause();
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
            <Button variant="secondary" disabled={disabled}>Add Audio<Mic/></Button>
        </DialogTrigger>
        <VisuallyHidden>
            <DialogTitle>Record Audio</DialogTitle>
            <DialogDescription>
                Records audio to be stored with a journal entry
            </DialogDescription>
        </VisuallyHidden>
        <DialogContent>
            <div className="flex flex-row flex-nowrap items-center justify-center gap-x-4">
                <Button
                    type="button"
                    variant="outline"
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
                    disabled={!recording_started}
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

export interface PlayAudioProps {
    src: string | File | Blob
}

export function PlayAudio({src}: PlayAudioProps) {
    let url = useObjectUrl(src);

    return <Dialog>
        <DialogTrigger asChild>
            <Button type="button" variant="secondary" size="icon">
                <SquareArrowOutUpRight/>
            </Button>
        </DialogTrigger>
        <VisuallyHidden>
            <DialogTitle>Play Audio</DialogTitle>
            <DialogDescription>
                Plays audio files attached to a journal entry
            </DialogDescription>
        </VisuallyHidden>
        <DialogContent>
            <audio controls src={url}/>
        </DialogContent>
    </Dialog>;
}
