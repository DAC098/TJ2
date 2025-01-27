import { format, formatDistanceToNow } from "date-fns";
import { useRef, useState, useEffect, useMemo, JSX } from "react";
import { Link, useParams, useNavigate } from "react-router-dom";
import { Plus, CalendarIcon, Trash, Save, ArrowLeft, Mic, Video, Download, Search, RefreshCw } from "lucide-react";
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
    Entry,
    EntryFileForm,
    EntryForm,
    EntryTagForm,
    now_date,
    get_date,
    blank_form,
    entry_to_form,
    retrieve_entry,
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
    const form = useFormContext<EntryForm>();

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

type EntryFileFormMap = {[key: string]: EntryFileForm};

function create_file_map(files: EntryFileForm[]): EntryFileFormMap {
    let rtn = {};

    for (let file of files) {
        if (file.type === "server") {
            continue;
        }

        rtn[file.key] = file;
    }

    return rtn;
}

async function parallel_uploads(
    journals_id: string | number,
    entry_form: EntryForm,
    entry: EntryForm,
): Promise<EntryFileForm[]> {
    let mapped = create_file_map(entry_form.files);
    let to_upload = [];
    let uploaders = [];

    for (let file_entry of entry.files) {
        if (file_entry.type !== "server") {
            continue;
        }

        if (file_entry.attached == null) {
            continue;
        }

        let ref = mapped[file_entry.attached.key];

        if (ref == null) {
            continue;
        }

        to_upload.push([file_entry, ref]);
    }

    let failed = [];

    for (let index = 0; index < 2; index += 1) {
        uploaders.push((async () => {
            let count = 0;

            while (true) {
                let uploading = to_upload.pop();

                if (uploading == null) {
                    console.log("uploader:", index, "finished. sent:", count);

                    break;
                }

                let [file_entry, ref] = uploading;

                console.log("uploader:", index, "sending file:", file_entry._id);

                try {
                    let successful = await upload_data(
                        journals_id,
                        entry.id,
                        file_entry._id,
                        ref
                    );

                    if (successful) {
                        console.log("file uploaded");
                    } else {
                        console.log("file upload failed");

                        failed.push(ref);
                    }
                } catch (err) {
                    console.log("file failed", err);

                    failed.push(ref);
                }

                count += 1;
            }
        })());
    }

    await Promise.all(uploaders);

    return failed;
}

export function Entry() {
    const { journals_id, entries_id } = useParams();
    const navigate = useNavigate();

    const form = useForm<EntryForm>({
        defaultValues: async () => {
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
        },
        disabled: false,
    });

    const create_and_upload = async (entry: EntryForm): Promise<[EntryForm, EntryFileForm[]]> => {
        let result = await create_entry(journals_id, entry);

        if (result.files.length === 0) {
            return [result, []];
        }

        let failed_uploads = await parallel_uploads(journals_id, entry, result);

        return [result, failed_uploads];
    };

    const update_and_upload = async (entry: EntryForm): Promise<[EntryForm, EntryFileForm[]]> => {
        let result = await update_entry(journals_id, entries_id, entry);

        if (result.files.length === 0) {
            return [result, []];
        }

        let failed_uploads = await parallel_uploads(journals_id, entry, result);

        return [result, failed_uploads];
    };

    const onSubmit: SubmitHandler<EntryForm> = async (data, event) => {
        if (entries_id == null || entries_id == "new") {
            try {
                let [result, failed] = await create_and_upload(data);

                console.log("created entry:", result, failed);

                if (failed.length === 0) {
                    form.reset(result);

                    navigate(`/journals/${journals_id}/entries/${result.id}`);
                }
            } catch(err) {
                console.error("failed to create entry:", err);
            }
        } else {
            try {
                let [result, failed] = await update_and_upload(data);

                console.log("updated entry:", result, failed);

                if (failed.length === 0) {
                    form.reset(result);
                }
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
        <FormProvider<EntryForm> {...form} children={
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
    const form = useFormContext<EntryForm>();
    const tags = useFieldArray<EntryForm, "tags">({
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

interface FileEntryProps {
    journals_id: string,
    entries_id: string,
}

function FileEntry({journals_id, entries_id}: FileEntryProps) {
    const form = useFormContext<EntryForm>();
    const files = useFieldArray<EntryForm, "files">({
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
                        mime_type,
                        mime_subtype,
                        mime_param,
                    });
                }
            }}/>
            <RecordAudio on_created={(blob) => {
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
            <RecordVideo on_created={(blob) => {
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
                let src = `/journals/${journals_id}/entries/${entries_id}/${field._id}`;

                download = <DownloadBtn src={src}/>;

                switch (field.mime_type) {
                case "audio":
                    player = <PlayAudio src={src}/>;
                    break;
                case "video":
                    player = <PlayVideo src={src}/>;
                    break;
                case "image":
                    player = <ViewImage src={src}/>;
                    break;
                }

                break;
            case "in-memory":
                download = <DownloadBtn src={field.data} name={field.name}/>;

                switch (field.mime_type) {
                case "audio":
                    player = <PlayAudio src={field.data}/>;
                    break;
                case "video":
                    player = <PlayVideo src={field.data}/>;
                    break;
                }

                break;
            case "local":
                download = null;

                switch (field.mime_type) {
                case "audio":
                    player = <PlayAudio src={field.data}/>;
                    break;
                case "video":
                    player = <PlayVideo src={field.data}/>;
                    break;
                case "image":
                    player = <ViewImage src={field.data}/>;
                    break;
                }

                break;
            }

            return <div key={field.id} className="flex flex-row flex-nowrap gap-x-4">
                <FormField control={form.control} name={`files.${index}.name`} render={({field: file_field}) => {
                    return <FormItem className="w-2/4">
                        <FormControl>
                            <Input type="text" {...file_field}/>
                        </FormControl>
                    </FormItem>
                }}/>
                {download}
                {player}
                <Button type="button" variant="destructive" size="icon" onClick={() => {
                    files.remove(index);
                }}><Trash/></Button>
            </div>;
        })}
    </div>;
}
