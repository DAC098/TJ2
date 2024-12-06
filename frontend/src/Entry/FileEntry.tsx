import { useContext, useEffect, useRef, useState } from "react";
import { useFormContext, useFieldArray } from "react-hook-form";
import { Mic, Video, Plus, Trash, Download } from "lucide-react";

import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
    FormControl,
    FormField,
    FormItem,
    FormLabel,
} from "@/components/ui/form";
import { uuidv4 } from "@/uuid";
import { getUserMedia } from "@/media";
import { EntryForm, LocalFile, timestamp_name } from "@/journal";
import { RecordAudio, PlayAudio } from "@/Entry/audio";
import { RecordVideo, PlayVideo } from "@/Entry/video";
import { ViewImage } from "@/Entry/image";
import { useObjectUrl } from "@/hooks";

interface AddFileProps {
    on_selected: (FileList) => void,
    disabled?: boolean
}

function AddFile({on_selected, disabled}: AddFileProps) {
    let input_ref = useRef<HTMLInputElement>(null);

    return <>
        <input
            ref={input_ref}
            type="file"
            multiple
            style={{display: "none"}}
            onChange={e => {
                on_selected(e.target.files);
            }}
        />
        <Button type="button" variant="secondary" disabled={disabled} onClick={() => {
            if (input_ref.current != null) {
                input_ref.current.click();
            }
        }}>
            Add File(s)<Plus/>
        </Button>
    </>
}

interface DownloadBtnProps {
    src: string | File | Blob,
    name?: string
}

function DownloadBtn({src, name}: DownloadBtnProps) {
    let url = useObjectUrl(src);

    return <a href={url} download={name ?? true}>
        <Button type="button" variant="secondary" size="icon">
            <Download/>
        </Button>
    </a>;
}

interface FileEntryProps {
    entries_id: string,
    loading: boolean,
}

const FileEntry = ({entries_id, loading}: FileEntryProps) => {
    const form = useFormContext<EntryForm>();
    const files = useFieldArray<EntryForm, "files">({
        control: form.control,
        name: "files"
    });

    useEffect(() => {
        return () => {
            console.log(files.fields);
        }
    }, []);

    return <div className="space-y-4">
        <div className="flex flex-row flex-nowrap gap-x-4 items-center">
            Files
            <AddFile disabled={loading} on_selected={file_list => {
                for (let file of file_list) {
                    // more than likely this is not correct
                    let mime_split = file.type.split("/");
                    let mime_type = mime_split[0];
                    let mime_subtype = mime_split[1];
                    let mime_param: null;

                    files.append({
                        type: "local",
                        key: uuidv4(),
                        name: file.name,
                        data: file,
                        mime_type,
                        mime_subtype,
                        mime_param,
                    });
                }
            }}/>
            <RecordAudio disabled={loading} on_created={(blob) => {
                files.append({
                    type: "in-memory",
                    key: uuidv4(),
                    data: blob,
                    name: `${timestamp_name()}_audio`,
                    mime_type: "audio",
                    mime_subtype: "webm",
                    mime_param: null,
                });
            }}/>
            <RecordVideo disabled={loading} on_created={(blob) => {
                files.append({
                    type: "in-memory",
                    key: uuidv4(),
                    data: blob,
                    name: `${timestamp_name()}_video`,
                    mime_type: "video",
                    mime_subtype: "webm",
                    mime_param: null,
                });
            }}/>
        </div>
        {files.fields.map((field, index) => {
            let download;
            let player;

            switch (field.type) {
            case "server":
                let src = `/entries/${entries_id}/${field._id}`;

                download = <DownloadBtn src={src}/>;

                switch (field.mime_type) {
                case "audio":
                    player = <PlayAudio src={src}/>
                    break;
                case "video":
                    player = <PlayVideo src={src}/>
                    break;
                case "image":
                    player = <ViewImage src={src}/>
                    break;
                }

                break;
            case "in-memory":
                download = <DownloadBtn src={field.data} name={field.name}/>;

                switch (field.mime_type) {
                case "audio":
                    player = <PlayAudio src={field.data}/>
                    break;
                case "video":
                    player = <PlayVideo src={field.data}/>
                    break;
                }

                break;
            case "local":
                download = null;

                switch (field.mime_type) {
                case "audio":
                    player = <PlayAudio src={field.data}/>
                    break;
                case "video":
                    player = <PlayVideo src={field.data}/>
                    break;
                case "image":
                    player = <ViewImage src={field.data}/>
                    break;
                }

                break;
            }

            return <div key={field.id} className="flex flex-row flex-nowrap gap-x-4">
                <FormField control={form.control} name={`files.${index}.name`} render={({field: file_field}) => {
                    return <FormItem className="w-2/4">
                        <FormControl>
                            <Input type="text" disabled={loading} {...file_field}/>
                        </FormControl>
                    </FormItem>
                }}/>
                {download}
                {player}
                <Button type="button" variant="destructive" size="icon" disabled={loading} onClick={() => {
                    files.remove(index);
                }}><Trash/></Button>
            </div>
        })}
    </div>
}

export default FileEntry;
