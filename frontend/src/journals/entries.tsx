import { format, formatDistanceToNow } from "date-fns";
import { useRef, useState, useEffect, useMemo, JSX } from "react";
import { Link, useParams, useNavigate } from "react-router-dom";
import { Plus, CalendarIcon, Trash, Save, ArrowLeft, Mic, Video, Download, Search, RefreshCw, Info } from "lucide-react";
import { useForm, useFieldArray, useFormContext, FormProvider, SubmitHandler,  } from "react-hook-form";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Calendar } from "@/components/ui/calendar";
import {
    FormControl,
    FormDescription,
    FormField,
    FormItem,
    FormLabel,
    FormMessage,
} from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { CenterPage } from "@/components/ui/page";
import {
    Popover,
    PopoverContent,
    PopoverTrigger,
} from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import {
    DataTable,
    ColumnDef,
} from "@/components/ui/table";
import { Textarea } from "@/components/ui/textarea";
import {
    EntryPartial,
    FailedFile,
    InMemoryFile,
    LocalFile,
    RequestedFile,
    ReceivedFile,
    EntryFileForm,
    UIEntryFileForm,
    EntryForm,
    UIEntryForm,
    EntryTagForm,
    // functions
    now_date,
    get_date,
    blank_form,
    create_entry,
    update_entry,
    delete_entry,
    upload_data,
    timestamp_name,
    naive_date_to_date,
    custom_field,
} from "@/journals/api";
import { RecordAudio, PlayAudio } from "@/journals/audio";
import { CustomFieldEntries, CustomFieldEntryCell } from "@/journals/custom_fields";
import { RecordVideo, PlayVideo } from "@/journals/video";
import { ViewImage } from "@/journals/image";
import { getUserMedia } from "@/media";
import { useObjectUrl } from "@/hooks";
import { cn } from "@/utils";
import { uuidv4 } from "@/uuid";
import { parse_mime } from "@/parse";

interface CustomFieldPartial {
    id: number,
    name: string,
    description: string | null,
    config: custom_field.Type,
}

export function Entries() {
    const { journals_id } = useParams();

    let [loading, set_loading] = useState(false);
    let [{entries, custom_fields}, set_list_data] = useState<{
        entries: EntryPartial[],
        custom_fields: CustomFieldPartial
    }>({
        entries: [],
        custom_fields: [],
    });

    const search_entries = async () => {
        set_loading(true);

        try {
            let res = await fetch(`/journals/${journals_id}/entries`);

            switch (res.status) {
            case 200:
                let json = await res.json();

                set_list_data({
                    entries: json.entries,
                    custom_fields: json.custom_fields,
                });
                break;
            default:
                console.log("unhandled response status");
            }
        } catch (err) {
            console.error("error when requesting entries", err);
        }

        set_loading(false);
    };

    useEffect(() => {
        search_entries();
    }, [journals_id]);

    const columns = useMemo(() => {
        let columns: ColumnDef<EntryPartial>[] = [
            {
                header: "Date",
                cell: ({ row }) => {
                    return <Link to={`/journals/${row.original.journals_id}/entries/${row.original.id}`}>
                        {row.original.date}
                    </Link>;
                }
            },
            {
                accessorKey: "title",
                header: "Title",
            },
        ];

        for (let field of custom_fields) {
            columns.push({
                header: field.name,
                cell: ({ row }) => {
                    if (!(field.id in row.original.custom_fields)) {
                        return null;
                    }

                    return <CustomFieldEntryCell
                        value={row.original.custom_fields[field.id]}
                        config={field.config}
                    />;
                }
            });
        }

        columns.push(
            {
                header: "Tags",
                cell: ({ row }) => {
                    let list = [];

                    for (let tag in row.original.tags) {
                        let value = row.original.tags[tag];

                        list.push(<Badge key={tag} variant="outline" title={value}>
                            {tag}
                        </Badge>);
                    }

                    return <div className="max-w-96 flex flex-row flex-wrap gap-1">{list}</div>;
                }
            },
            {
                header: "Mod",
                cell: ({ row }) => {
                    let to_use = row.original.updated != null ?
                        new Date(row.original.updated) :
                        new Date(row.original.created);
                    let distance = formatDistanceToNow(to_use, {
                        addSuffix: true,
                        includeSeconds: true,
                    });

                    return <span title={to_use} className="text-nowrap">{distance}</span>;
                }
            }
        );

        return columns;
    }, [custom_fields]);

    return <CenterPage className="pt-4 max-w-6xl">
        <div className="flex flex-row flex-nowrap gap-x-4">
            <div className="w-1/2 relative">
                <Input type="text" placeholder="Search" className="pr-10"/>
                <Button type="button" variant="ghost" size="icon" className="absolute right-0 top-0">
                    <Search/>
                </Button>
            </div>
            <Button type="button" variant="secondary" size="icon" onClick={() => {
                search_entries();
            }}>
                <RefreshCw />
            </Button>
            <Link to={`/journals/${journals_id}/entries/new`}>
                <Button type="button"><Plus/>New Entry</Button>
            </Link>
        </div>
        <DataTable columns={columns} data={entries}/>
    </CenterPage>;
}

interface EntrySecProps {
    title: JSX.Element,
    children?: JSX.Element[] | JSX.Element
}

const EntrySec = ({title, children}: EntrySecProps) => {
    return <>
        <div>{title}</div>
        <div>{children}</div>
    </>
};

interface EntrySecTitleProps {
    title: String
}

const EntrySecTitle = ({title}: EntrySecTitleProps) => {
    return <div className="text-right w-full">{title}</div>
};

interface EntryHeaderProps {
    journals_id: string,
    entries_id: string,
    loading: boolean
}

function EntryHeader({journals_id, entries_id, loading}: EntryHeaderProps) {
    const navigate = useNavigate();
    const form = useFormContext<UIEntryForm>();

    return <div className="top-0 sticky flex flex-row flex-nowrap gap-x-4 bg-background border-b py-2">
        <Link to={`/journals/${journals_id}/entries`}>
            <Button type="button" variant="ghost" size="icon">
                <ArrowLeft/>
            </Button>
        </Link>
        <FormField control={form.control} name="date" render={({field}) => {
            let date_value = typeof field.value === "string" ? naive_date_to_date(field.value) : field.value;

            return <FormItem>
                <Popover>
                    <PopoverTrigger asChild>
                        <FormControl>
                            <Button
                                variant="outline"
                                className={cn("w-[280px] justify-start text-left front-normal")}
                            >
                                {format(date_value, "PPP")}
                                <CalendarIcon className="mr-2 h-4 w-4"/>
                            </Button>
                        </FormControl>
                    </PopoverTrigger>
                    <PopoverContent className="w-auto p-0" aligh="start">
                        <Calendar
                            name={field.name}
                            mode="single"
                            selected={date_value}
                            onBlur={field.onBlur}
                            onSelect={field.onChange}
                            disabled={(date) => {
                                return date > new Date() || date < new Date("1900-01-01");
                            }}
                            initialFocus
                        />
                    </PopoverContent>
                </Popover>
                <FormMessage />
            </FormItem>
        }}/>
        <Button type="submit" disabled={loading}>Save<Save/></Button>
        {entries_id != null && entries_id !== "new" ?
            <Button type="button" variant="destructive" disabled={loading} onClick={() => {
                delete_entry(journals_id, entries_id).then(() => {
                    navigate(`/journals/${journals_id}/entries`);
                }).catch(err => {
                    console.error("failed to delete journal entry:", err);
                });
            }}>Delete<Trash/></Button>
            :
            null
        }
    </div>;
};

interface UploadResult {
    successful: ReceivedFile[],
    failed: FailedFile[],
}

async function parallel_uploads(
    journals_id: string | number,
    entries_id: string | number,
    local: UIEntryFileForm[],
    server: EntryFileForm[],
): Promise<UploadResult> {
    let to_skip = {};
    let mapped: {[key: string]: InMemoryFile | LocalFile} = {};
    let to_upload: [RequestedFile, InMemoryFile | LocalFile][] = [];
    let uploaders = [];

    for (let file of local) {
        switch (file.type) {
            case "received":
            case "requested":
                continue;
            case "local":
            case "in-memory":
                mapped[file.key] = file;
                break;
            case "failed":
                to_skip[file._id] = 1;

                to_upload.push([{
                    type: "requested",
                    _id: file._id,
                    uid: file.uid,
                    name: file.name,
                }, file.original]);
                break;
        }
    }

    let result = {
        successful: [],
        failed: [],
    };

    for (let file_entry of server) {
        if (file_entry.type === "received") {
            result.successful.push(file_entry);

            continue;
        }

        if (file_entry.attached == null) {
            if (!(file_entry._id in to_skip)) {
                result.successful.push(file_entry);
            }

            continue;
        }

        let ref = mapped[file_entry.attached.key];

        if (ref == null) {
            throw new Error("failed to find file reference, THIS SHOULD NOT HAPPEN");
        }

        to_upload.push([file_entry, ref]);
    }

    for (let index = 0; index < 2; index += 1) {
        uploaders.push((async () => {
            let count = 0;

            while (true) {
                let uploading = to_upload.pop();

                if (uploading == null) {
                    break;
                }

                let [file_entry, ref] = uploading;

                try {
                    let [successful, json] = await upload_data(
                        journals_id,
                        entries_id,
                        file_entry._id,
                        ref.data
                    );

                    if (successful) {
                        result.successful.push(json);
                    } else {
                        result.failed.push({
                            type: "failed",
                            _id: file_entry._id,
                            uid: file_entry.uid,
                            name: file_entry.name,
                            original: ref,
                        });
                    }
                } catch (err) {
                    result.failed.push({
                        type: "failed",
                        _id: file_entry._id,
                        uid: file_entry.uid,
                        name: file_entry.name,
                        original: ref
                    });
                }

                count += 1;
            }
        })());
    }

    await Promise.all(uploaders);

    return result;
}

async function retrieve_entry(
    journals_id: string | number,
    entries_id: string | number
) {
    try {
        let res = await fetch(`/journals/${journals_id}/entries/${entries_id}`);

        switch (res.status) {
            case 200: {
                let json = await res.json();

                if (entries_id === "new") {
                    json.date = now_date();
                }

                return json;
            }
            default: {
                let json = await res.json();

                console.log("failed to retrieve entry for journal", json);

                break;
            }
        }
    } catch (err) {
        console.log("failed to retrieve entry", err);
    }

    return blank_form();
}

export function Entry() {
    const { journals_id, entries_id } = useParams();
    const navigate = useNavigate();

    const form = useForm<UIEntryForm>({
        defaultValues: async () => retrieve_entry(journals_id, entries_id),
        disabled: false,
    });

    const create_and_upload = async (entry: UIEntryForm): Promise<[EntryForm, UploadResult]> => {
        let result = await create_entry(journals_id, entry);

        if (result.files.length === 0) {
            return [result, {successful: [], failed: []}];
        }

        let uploaded = await parallel_uploads(
            journals_id,
            result.id,
            entry.files,
            result.files
        );

        return [result, uploaded];
    };

    const update_and_upload = async (entry: UIEntryForm): Promise<[EntryForm, UploadResult]> => {
        let result = await update_entry(journals_id, entries_id, entry);

        if (result.files.length === 0) {
            return [result, {successful: [], failed: []}];
        }

        let uploaded = await parallel_uploads(
            journals_id,
            result.id,
            entry.files,
            result.files
        );

        return [result, uploaded];
    };

    const onSubmit: SubmitHandler<UIEntryForm> = async (data, event) => {
        if (entries_id == null || entries_id == "new") {
            try {
                let [result, uploaded] = await create_and_upload(data);

                console.log("created entry:", result, uploaded.failed);

                (result as UIEntryForm).files = [
                    ...uploaded.successful,
                    ...uploaded.failed
                ];

                form.reset(result);

                navigate(`/journals/${journals_id}/entries/${result.id}`);
            } catch(err) {
                console.error("failed to create entry:", err);
            }
        } else {
            try {
                let [result, uploaded] = await update_and_upload(data);

                console.log("updated entry:", result, uploaded);

                (result as UIEntryForm).files = [
                    ...uploaded.successful,
                    ...uploaded.failed
                ];

                form.reset(result);
            } catch(err) {
                console.error("failed to update entry:", err);
            }
        }
    };

    if (form.formState.isLoading) {
        return <div className="max-w-3xl mx-auto my-auto">
        </div>;
    }

    return <div className="max-w-3xl mx-auto my-auto">
        <FormProvider<UIEntryForm> {...form} children={
            <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
                <EntryHeader
                    journals_id={journals_id}
                    entries_id={entries_id}
                    loading={form.formState.isLoading || form.formState.isSubmitting}
                />
                <FormField control={form.control} name="title" render={({field, formState}) => {
                    return <FormItem className="w-2/4">
                        <FormLabel>Title</FormLabel>
                        <FormControl>
                            <Input
                                ref={field.ref}
                                name={field.name}
                                disabled={formState.isLoading || formState.isSubmitting || field.disabled}
                                value={field.value ?? ""}
                                onBlur={field.onBlur}
                                onChange={field.onChange}
                            />
                        </FormControl>
                    </FormItem>
                }}/>
                <FormField control={form.control} name="contents" render={({field, formState}) => {
                    return <FormItem className="w-3/4">
                        <FormLabel>Contents</FormLabel>
                        <FormControl>
                            <Textarea
                                ref={field.ref}
                                name={field.name}
                                disabled={formState.isLoading || formState.isSubmitting || field.disabled}
                                value={field.value ?? ""}
                                onBlur={field.onBlur}
                                onChange={field.onChange}
                            />
                        </FormControl>
                    </FormItem>
                }}/>
                <Separator/>
                <CustomFieldEntries />
                <Separator/>
                <TagEntry />
                <Separator/>
                <FileEntry journals_id={journals_id} entries_id={entries_id}/>
            </form>
        }/>
    </div>;
}

interface TagEntryProps {
}

function TagEntry({}: TagEntryProps) {
    const form = useFormContext<UIEntryForm>();
    const tags = useFieldArray<UIEntryForm, "tags">({
        control: form.control,
        name: "tags"
    });

    return <div className="space-y-4">
        <div className="flex flex-row flex-nowrap gap-x-4 items-center">
            Tags
            <Button type="button" variant="secondary" onClick={() => {
                tags.append({key: "", value: ""});
            }}>Add Tag<Plus/></Button>
        </div>
        {tags.fields.map((field, index) => {
            return <div key={field.id} className="flex flex-row flex-nowrap gap-x-4">
                <FormField control={form.control} name={`tags.${index}.key`} render={({field: tag_field}) => {
                    return <FormItem className="w-1/4">
                        <FormControl>
                            <Input type="text" {...tag_field}/>
                        </FormControl>
                    </FormItem>
                }}/>
                <FormField control={form.control} name={`tags.${index}.value`} render={({field: tag_field}) => {
                    return <FormItem className="w-2/4">
                        <FormControl>
                            <Input
                                ref={tag_field.ref}
                                name={tag_field.name}
                                disabled={tag_field.disabled}
                                value={tag_field.value ?? ""}
                                type="text"
                                onBlur={tag_field.onBlur}
                                onChange={tag_field.onChange}
                            />
                        </FormControl>
                    </FormItem>
                }}/>
                <Button type="button" variant="destructive" size="icon" onClick={() => {
                    tags.remove(index);
                }}><Trash/></Button>
            </div>;
        })}
    </div>;
}

interface AddFileProps {
    on_selected: (FileList) => void,
    disabled?: boolean
}

function AddFile({on_selected, disabled = false}: AddFileProps) {
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
    </>;
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

interface FilePreviewProps {
    mime_type: string,
    data: Blob | File | string
}

function FilePreview({mime_type, data}: FilePreviewProps) {
    switch (mime_type) {
    case "audio":
        return <PlayAudio src={data}/>;
    case "video":
        return <PlayVideo src={data}/>;
    case "image":
        return <ViewImage src={data}/>;
    }
}

interface WarnButtonProps {
    message: string
}

function WarnButton({message}: WarnButtonProps) {
    return <Popover>
        <PopoverTrigger asChild>
            <Button type="button" variant="secondary" size="icon" className="text-yellow-300">
                <Info/>
            </Button>
        </PopoverTrigger>
        <PopoverContent>
            {message}
        </PopoverContent>
    </Popover>;
}

interface FileEntryProps {
    journals_id: string,
    entries_id: string,
}

function FileEntry({journals_id, entries_id}: FileEntryProps) {
    const form = useFormContext<UIEntryForm>();
    const files = useFieldArray<UIEntryForm, "files">({
        control: form.control,
        name: "files"
    });

    return <div className="space-y-4">
        <div className="flex flex-row flex-nowrap gap-x-4 items-center">
            Files
            <AddFile on_selected={file_list => {
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
                    });
                }
            }}/>
            <RecordAudio on_created={(blob) => {
                files.append({
                    type: "in-memory",
                    key: uuidv4(),
                    data: blob,
                    name: `${timestamp_name()}_audio`,
                });
            }}/>
            <RecordVideo on_created={(blob) => {
                files.append({
                    type: "in-memory",
                    key: uuidv4(),
                    data: blob,
                    name: `${timestamp_name()}_video`,
                });
            }}/>
        </div>
        {files.fields.map((field, index) => {
            let download = null;
            let player = null;
            let status = null;

            switch (field.type) {
                case "requested":
                    status = <WarnButton message={"The file was never received by the server."}/>
                    break;
                case "received":
                    let src = `/journals/${journals_id}/entries/${entries_id}/${field._id}`;

                    download = <DownloadBtn src={`${src}?download=true`}/>;
                    player = <FilePreview mime_type={field.mime_type} data={src}/>
                    break;
                case "in-memory": {
                    let mime = parse_mime(field.data.type);

                    download = <DownloadBtn src={field.data} name={field.name}/>;
                    player = <FilePreview mime_type={mime.type} data={field.data}/>;
                    break;
                }
                case "local": {
                    let mime = parse_mime(field.data.type);

                    player = <FilePreview mime_type={mime.type} data={field.data}/>;
                    break;
                }
                case "failed": {
                    status = <WarnButton message={"There was an error when sending the file to the server."}/>;

                    switch (field.original.type) {
                        case "local": {
                            let mime = parse_mime(field.original.data.type);

                            player = <FilePreview mime_type={mime.type} data={field.original.data}/>;
                            break;
                        }
                        case "in-memory": {
                            let mime = parse_mime(field.original.data.type);

                            download = <DownloadBtn src={field.original.data} name={field.name}/>;
                            player = <FilePreview mime_type={mime.type} data={field.original.data}/>;
                            break;
                        }
                    }
                    break;
                }
            }

            return <div key={field.id} className="flex flex-row flex-nowrap gap-x-4">
                <FormField control={form.control} name={`files.${index}.name`} render={({field: file_field}) => {
                    return <FormItem className="w-2/4">
                        <FormControl>
                            <Input type="text" {...file_field}/>
                        </FormControl>
                    </FormItem>
                }}/>
                {status}
                {download}
                {player}
                <Button type="button" variant="destructive" size="icon" onClick={() => {
                    files.remove(index);
                }}><Trash/></Button>
            </div>;
        })}
    </div>;
}
