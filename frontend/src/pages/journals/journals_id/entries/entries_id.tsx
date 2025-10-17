import { format } from "date-fns";
import { SyntheticEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useParams, useNavigate } from "react-router-dom";
import { Plus, CalendarIcon, Trash, Save, ArrowLeft, Download, Info, Pencil, LoaderCircle, Fullscreen, X, ArrowRight } from "lucide-react";
import { useForm, useFieldArray, useFormContext, FormProvider, SubmitHandler } from "react-hook-form";

import { Button } from "@/components/ui/button";
import { Calendar } from "@/components/ui/calendar";
import {
    FormControl,
    FormField,
    FormItem,
    FormLabel,
    FormMessage,
} from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { CenterMessage, CenterPage, Loading } from "@/components/ui/page";
import {
    Popover,
    PopoverContent,
    PopoverTrigger,
} from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import { Textarea } from "@/components/ui/textarea";
import {
    FailedFile,
    InMemoryFile,
    LocalFile,
    RequestedFile,
    ReceivedFile,
    EntryFileForm,
    UIEntryFileForm,
    EntryForm,
    UIEntryForm,
    // functions
    create_entry,
    update_entry,
    delete_entry,
    upload_data,
    timestamp_name,
    naive_date_to_date,
    JournalFull,
    EntryCustomFieldForm,
    custom_field,
    EntryTagForm,
} from "@/journals/api";
import { RecordAudio, PlayAudio } from "@/components/audio";
import { EditCustomFieldEntries, EntryCustomField } from "@/journals/custom_fields";
import { RecordVideo, PlayVideo } from "@/components/video";
import { ViewImage } from "@/components/image";
import { useObjectUrl } from "@/hooks";
import { cn, merge_sorted } from "@/utils";
import { uuidv4 } from "@/uuid";
import { parse_mime, default_mime } from "@/parse";
import { ApiError, req_api_json } from "@/net";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ApiErrorMsg, ErrorMsg } from "@/components/error";
import { H1, H2, H4, P } from "@/components/ui/typeography";
import { useCurrJournal } from "@/components/hooks/journal";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Dialog, dialog_portal, DialogOverlay, DialogPortal } from "@/components/ui/dialog";

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
    let to_skip: {[key: number]: number} = {};
    let mapped: {[key: string]: InMemoryFile | LocalFile} = {};
    let to_upload: [RequestedFile, InMemoryFile | LocalFile][] = [];
    let uploaders = [];

    let result: UploadResult = {
        successful: [],
        failed: [],
    };

    for (let file of local) {
        switch (file.type) {
            case "requested":
            case "received":
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

    for (let file_entry of server) {
        if (file_entry.type !== "requested") {
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

    const uploader = async () => {
        let count = 0;

        while (true) {
            let uploading = to_upload.pop();

            if (uploading == null) {
                break;
            }

            let [file_entry, ref] = uploading;

            try {
                let json = await upload_data(
                    journals_id,
                    entries_id,
                    file_entry._id,
                    ref.data
                );

                if (json != null) {
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
    };

    for (let index = 0; index < 2; index += 1) {
        uploaders.push(uploader());
    }

    await Promise.all(uploaders);

    return result;
}

async function retrieve_entry(journals_id: string | number, entries_id: string | number) {
    try {
        return await req_api_json<EntryForm>("GET", `/journals/${journals_id}/entries/${entries_id}`);
    } catch (err) {
        if (err instanceof ApiError) {
            if (err.kind === "EntryNotFound") {
                return null;
            }

            throw err;
        } else {
            throw err;
        }
    }
}

const ENTRY_ID_NUMBER = 0;
const ENTRY_ID_DATE = 1;
const ENTRY_ID_NEW = 2;

interface EntryIdNew {
    type: typeof ENTRY_ID_NEW,
}

interface EntryIdNumber {
    type: typeof ENTRY_ID_NUMBER,
    value: number
}

interface EntryIdDate {
    type: typeof ENTRY_ID_DATE,
    value: string,
    year: number,
    month: number,
    day: number,
    date: Date,
}

type EntryId = EntryIdNumber | EntryIdDate | EntryIdNew;

function useEntryIdParams() {
    const {entries_id} = useParams();

    if (entries_id == null) {
        throw new Error("missing entries_id");
    }

    let parsed = useMemo<EntryId>(() => {
        if (entries_id === "new") {
            return {
                type: ENTRY_ID_NEW,
            };
        }

        let as_date = naive_date_to_date(entries_id);

        if (as_date != null) {
            return {
                type: ENTRY_ID_DATE,
                value: entries_id,
                year: as_date.getFullYear(),
                month: as_date.getMonth(),
                day: as_date.getDate(),
                date: as_date,
            };
        } else {
            return {
                type: ENTRY_ID_NUMBER,
                value: parseInt(entries_id, 10)
            };
        }
    }, [entries_id]);

    return parsed;
}

function retrieve_entry_query_key(journals_id: number, entries_id: EntryId): ["retrieve_entry", number, EntryId] {
    return ["retrieve_entry", journals_id, entries_id]
}

export function Entry() {
    const entries_id = useEntryIdParams();
    const navigate = useNavigate();

    const [view_edit, set_view_edit] = useState(false);

    const {
        id: journals_id,
        journal,
        is_fetching: fetching_journal,
        error: journal_error,
    } = useCurrJournal();

    const client = useQueryClient();
    const {
        data,
        isFetching: fetching_entry,
        error: entry_error,
    } = useQuery({
        queryKey: retrieve_entry_query_key(journals_id ?? 0, entries_id),
        queryFn: async ({queryKey}) => {
            const [_, journals_id, entries_id] = queryKey;

            if (entries_id.type === ENTRY_ID_NEW) {
                return null;
            }

            let result = await retrieve_entry(journals_id, entries_id.value);

            if (result != null) {
                // this needs to be better, for now we will "cast" this as a
                // string for ts to accept it
                result.date = naive_date_to_date(result.date as unknown as string);
            }

            return result;
        },
    });

    if (journals_id == null) {
        return <CenterMessage title="Invalid Journal Id">
            <P>The specified journal id is not valid</P>
            <Link to="/journals" className="mt-2">
                <Button type="button">
                    Back to journals
                </Button>
            </Link>
        </CenterMessage>;
    }

    if (fetching_journal || fetching_entry) {
        return <Loading title="Loading Entry"/>;
    }

    // display possible journal states
    if (journal_error != null) {
        if (journal_error instanceof ApiError) {
            return <ApiErrorMsg err={journal_error}/>;
        } else {
            return <ErrorMsg title="Failed to load journal"/>;
        }
    }

    if (journal == null) {
        return <CenterMessage title="Journal not found">
            <Link to="/journals">
                <Button type="button">
                    Back to journals
                </Button>
            </Link>
        </CenterMessage>;
    }

    // display possible entry states
    if (entry_error) {
        if (entry_error instanceof ApiError) {
            return <ApiErrorMsg err={entry_error}/>;
        } else {
            return <ErrorMsg title="Failed to retrieve entry"/>;
        }
    } else if (data == null) {
        if (!view_edit && entries_id.type === ENTRY_ID_DATE) {
            return <CenterMessage title="No Entry">
                <P>There was no entry found.</P>
                <div className="flex flex-row items-center gap-x-2 mt-2">
                    <Button type="button" variant="ghost" onClick={() => navigate(-1)}>
                        <ArrowLeft/> Go Back
                    </Button>
                    <Button type="button" onClick={() => set_view_edit(true)}>
                        <Plus/> New Entry
                    </Button>
                </div>
            </CenterMessage>;
        } else {
            return <EditEntry
                journal={journal}
                entries_id={entries_id}
                data={blank_form(journal, entries_id)}
                on_cancel={() => set_view_edit(false)}
                on_created={data => {
                    if (entries_id.type === ENTRY_ID_NUMBER) {
                        navigate(`/journals/${journals_id}/entries/${data.id}`, {replace: true});
                    } else {
                        client.setQueryData(retrieve_entry_query_key(journals_id, entries_id), data);
                        set_view_edit(false);
                    }
                }}
                on_updated={data => {
                    client.setQueryData(retrieve_entry_query_key(journals_id, entries_id), data);
                    set_view_edit(false);
                }}
            />;
        }
    } else {
        if (view_edit) {
            return <EditEntry
                journal={journal}
                entries_id={entries_id}
                data={entry_form(journal, data)}
                on_cancel={() => set_view_edit(false)}
                on_created={data => {
                    client.setQueryData(retrieve_entry_query_key(journals_id, entries_id), data);
                    set_view_edit(false)
                }}
                on_updated={data => {
                    client.setQueryData(retrieve_entry_query_key(journals_id, entries_id), data);
                    set_view_edit(false);
                }}
            />;
        } else {
            return <ViewEntry
                journal={journal}
                entry={data}
                on_edit={() => set_view_edit(true)}
            />;
        }
    }
}

interface ViewEntryProps {
    journal: JournalFull,
    entry: EntryForm,
    on_edit: () => void,
}

function ViewEntry({journal, entry, on_edit}: ViewEntryProps) {
    const navigate = useNavigate();

    const {mutate, isPending} = useMutation({
        mutationFn: async ({journals_id, entries_id}: {journals_id: number, entries_id: number}) => {
            await delete_entry(journals_id, entries_id);
        },
        onSuccess: (data, vars, ctx) => {
            navigate(-1);
        },
        onError: (err, vars, ctx) => {
            if (err instanceof ApiError) {
                toast(`Failed to delete journal entry: ${err.kind}`);
            } else {
                toast(`Failed to delete journal entry: ClientError`);
            }
        }
    });

    return <CenterPage>
        <div className="top-0 sticky flex flex-row items-center flex-nowrap gap-x-4 bg-background border-b py-2">
            <Button type="button" variant="ghost" size="icon" onClick={() => navigate(-1)}>
                <ArrowLeft/>
            </Button>
            <span>{format(entry.date, "PPPP")}</span>
            <div className="flex-1"/>
            <Button type="button" onClick={() => on_edit()}>
                Edit <Pencil/>
            </Button>
            <Button
                type="button"
                variant="destructive"
                disabled={isPending}
                onClick={() => mutate({journals_id: journal.id, entries_id: entry.id!})}
            >
                Delete <Trash/>
            </Button>
        </div>
        {entry.title?.length !== 0 ? <H1>{entry.title}</H1> : null}
        <ViewEntryContents contents={entry.contents}/>
        <ViewEntryTags tags={entry.tags}/>
        <Separator/>
        <ViewEntryCustomFields fields={entry.custom_fields}/>
        <Separator/>
        <ViewEntryFiles journal={journal} entry={entry} files={entry.files}/>
    </CenterPage>;
}

interface ViewEntryContentsProps {
    contents: string | null
}

function ViewEntryContents({contents}: ViewEntryContentsProps) {
    let elements = useMemo(() => {
        if (contents == null) {
            return [];
        }

        let rtn = [];
        let min = 10;

        for (let segment of contents.split(/\n/)) {
            let key = segment.slice(0, segment.length > min ? min : segment.length);

            rtn.push(<P key={key}>{segment}</P>);
        }

        return rtn;
    }, [contents]);

    return <>{elements}</>;
}

interface ViewEntryCustomFieldsProps {
    fields: EntryCustomFieldForm[]
}

function ViewEntryCustomFields({fields}: ViewEntryCustomFieldsProps) {
    return <div className="grid grid-cols-2 gap-2">
        {fields.map(value => {
            if (value.enabled) {
                return <EntryCustomField key={value._id} field={value}/>;
            } else {
                return null;
            }
        })}
    </div>
}

interface ViewEntryTagsProps {
    tags: EntryTagForm[]
}

function ViewEntryTags({tags}: ViewEntryTagsProps) {
    return <div className="flex flex-row items-center gap-2">
        {tags.map(tag => {
            return tag.value != null ?
                <Tooltip key={tag.key}>
                    <TooltipTrigger>
                        <Badge variant="outline">{tag.key}</Badge>
                    </TooltipTrigger>
                    <TooltipContent>
                        <p>{tag.value}</p>
                    </TooltipContent>
                </Tooltip>
                :
                <div key={tag.key}>
                    <Badge key={tag.key} variant="outline">{tag.key}</Badge>
                </div>
        })}
    </div>
}

interface ViewEntryFilesProps {
    journal: JournalFull,
    entry: EntryForm,
    files: EntryFileForm[],
}

function ViewEntryFiles({journal, entry, files}: ViewEntryFilesProps) {
    let [preview, received, requested] = useMemo(() => {
        let preview = [];
        let received = [];
        let requested = [];

        for (let file of files) {
            if (file.type === "received") {
                if (file.mime_type === "image") {
                    preview.push(file);
                } else {
                    received.push(file);
                }
            } else if (file.type === "requested") {
                requested.push(file);
            }
        }

        return [preview, received, requested];
    }, []);

    return <>
        <PreviewList journal={journal} entry={entry} files={preview}/>
        <div className="flex flex-col">{received.map(file => {
            let src = `/journals/${journal.id}/entries/${entry.id}/${file._id}?download=true`;

            return <div key={file._id} className="flex flex-row items-center gap-x-2">
                <span>{file.name}</span>
                <DownloadBtn src={src} name={file.name}/>
            </div>
        })}</div>
        <div className="flex flex-col">{requested.map(file => {
            return <div key={file._id} className="flex flex-row items-center gap-x-2">
                <span>{file.name}</span>
                <WarnButton message={"The file was never received by the server."}/>
            </div>;
        })}</div>
    </>
}

interface PreviewListProps {
    journal: JournalFull,
    entry: EntryForm,
    files: ReceivedFile[],
}

function PreviewList({journal, entry, files}: PreviewListProps) {
    let [full_view, set_full_view] = useState<number | null>(null);

    function inc_full_view() {
        set_full_view(v => v != null ? (v + 1) % files.length : null);
    }

    function dec_full_view() {
        set_full_view(v => {
            if (v == null) {
                return null;
            } else {
                return v === 0 ? files.length - 1 : v - 1;
            }
        });
    }

    useEffect(() => {
        if (full_view == null) {
            return;
        }

        const controller = new AbortController();

        window.addEventListener("keydown", ev => {
            if (ev.key === "Escape") {
                set_full_view(null);
            } else if (ev.key === "ArrowLeft") {
                dec_full_view();
            } else if (ev.key === "ArrowRight") {
                inc_full_view();
            }
        }, { signal: controller.signal });

        return () => {
            controller.abort();
        }
    }, [full_view]);

    let list = useMemo(() => {
        let list = [];

        for (let index = 0; index < files.length; index += 1) {
            let file = files[index];
            let src = `/journals/${journal.id}/entries/${entry.id}/${file._id}`;
            let download = src + "?download=true";

            if (file.mime_type === "image") {
                list.push(<PreviewImage
                    key={file._id}
                    name={file.name}
                    src={src}
                    download={download}
                    on_fullscreen={() => set_full_view(index)}
                />);
            } else {
                list.push(<PreviewFile
                    name={file.name}
                    download={download}
                    on_fullscreen={() => set_full_view(index)}
                />);
            }
        }

        return list;
    }, [files]);

    let fullscreen_content = null;

    if (full_view != null) {
        let src = `/journals/${journal.id}/entries/${entry.id}/${files[full_view]._id}`;
        let download = src + "?download=true";

        switch (files[full_view].mime_type) {
            case "image":
                fullscreen_content = <FullscreenImage
                    src={src}
                    download={download}
                />;
                break;
            default:
                fullscreen_content = <FullscreenFile
                    name={files[full_view].name}
                    download={download}
                />;
        }
    }

    return <>
        <div className="flex flex-row flex-wrap">{list}</div>
        <Dialog open={full_view != null} onOpenChange={value => {
            if (value) {
                set_full_view(0)
            } else {
                set_full_view(null);
            }
        }}>
            <DialogPortal container={dialog_portal}>
                <DialogOverlay>
                    {fullscreen_content}
                    <div className="absolute top-2 right-2">
                        <Button type="button" variant="ghost" size="icon" onClick={() => set_full_view(null)}>
                            <X/>
                            <span className="sr-only">Close</span>
                        </Button>
                    </div>
                    <div className="absolute top-1/2 left-2 -translate-y-1/2">
                        <Button type="button" variant="ghost" size="icon" onClick={() => dec_full_view()}>
                            <ArrowLeft/>
                            <span className="sr-only">Left</span>
                        </Button>
                    </div>
                    <div className="absolute top-1/2 right-2 -translate-y-1/2">
                        <Button type="button" variant="ghost" size="icon" onClick={() => inc_full_view()}>
                            <ArrowRight/>
                            <span className="sr-only">Right</span>
                        </Button>
                    </div>
                </DialogOverlay>
            </DialogPortal>
        </Dialog>
    </>
}

interface PreviewImageProps {
    name: string,
    src: string,
    download: string,
    on_fullscreen: () => void,
}

function PreviewImage({name, src, download, on_fullscreen}: PreviewImageProps) {
    let [is_loading, set_is_loading] = useState(true);

    const on_load = useCallback((ev: SyntheticEvent<HTMLImageElement, Event>) => {
        if ((ev.target as HTMLImageElement).complete) {
            set_is_loading(false);
        }
    }, []);

    return <Tooltip>
        <TooltipTrigger asChild>
            <div className="relative w-40 h-40 p-1">
                <img
                    src={src}
                    className={cn("w-full h-full object-cover rounded-lg", {"hidden": is_loading})}
                    onLoad={on_load}
                />
                <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2"
                    onClick={() => on_fullscreen()}
                >
                    {is_loading ? <LoaderCircle className="animate-spin"/> : <Fullscreen/>}
                </Button>
                <a href={download} download className="absolute bottom-1 right-1">
                    <Button type="button" variant="ghost" size="icon">
                        <Download/>
                    </Button>
                </a>
            </div>
        </TooltipTrigger>
        <TooltipContent>
            <P>{name}</P>
        </TooltipContent>
    </Tooltip>
}

interface PreviewFileProps {
    name: string,
    download: string,
    on_fullscreen: () => void,
}

function PreviewFile({name, download, on_fullscreen}: PreviewFileProps) {
    return <div className="relative w-40 h-40 p-1 flex flex-col items-center justify-center">
        <H4>{name}</H4>
        <Button
            type="button"
            variant="ghost"
            size="icon"
            onClick={() => on_fullscreen()}
        >
            <Fullscreen/>
        </Button>
        <a href={download} download className="absolute bottom-1 right-1">
            <Button type="button" variant="ghost" size="icon">
                <Download/>
            </Button>
        </a>
    </div>;
}

interface FullscreenImageProps {
    src: string,
    download: string,
}

function FullscreenImage({src, download}: FullscreenImageProps) {
    let [is_loaded, set_is_loaded] = useState(false);

    const on_load = useCallback((ev: SyntheticEvent<HTMLImageElement, Event>) => {
        let ele = ev.target as HTMLImageElement;

        if (ele.complete) {
            set_is_loaded(true);
        }
    }, []);

    useEffect(() => {
        set_is_loaded(false);
    }, [src]);

    return <>
        <div className="w-full h-full flex flex-col items-center justify-center">
            {is_loaded ? null : <LoaderCircle className="animate-spin"/>}
            <img src={src} className={cn("max-w-full max-h-full", {"hidden": !is_loaded})} onLoad={on_load}/>
        </div>
        <div className="absolute bottom-2 left-1/2 -translate-x-1/2 flex flex-row items-center justify-center">
            <a href={download} download>
                <Button type="button" variant="ghost" size="icon">
                    <Download/>
                </Button>
            </a>
        </div>
    </>
}

interface FullscreenFileProps {
    name: string,
    download: string,
}

function FullscreenFile({name, download}: FullscreenFileProps) {
    return <div className="w-full h-full flex flex-col items-center justify-center gap-2">
        <H2>{name}</H2>
        <a href={download} download>
            <Button type="button" variant="outline">
                Download <Download/>
            </Button>
        </a>
    </div>
}

function fields_from_journal(journal: JournalFull, to_skip: Set<number>) {
    let custom_fields: EntryCustomFieldForm[] = [];

    for (let field of journal.custom_fields) {
        if (to_skip.has(field.id)) {
            continue;
        }

        let item: any = {
            _id: field.id,
            uid: field.uid,
            name: field.name,
            enabled: false,
            description: field.description,
        };

        switch (field.config.type) {
            case custom_field.TypeName.Float:
                item.type = custom_field.TypeName.Float;
                item.config = field.config;
                item.value = custom_field.make_float(field.config);
                break;
            case custom_field.TypeName.FloatRange:
                item.type = custom_field.TypeName.FloatRange;
                item.config = field.config;
                item.value = custom_field.make_float_range(field.config);
                break;
            case custom_field.TypeName.Integer:
                item.type = custom_field.TypeName.Integer;
                item.config = field.config;
                item.value = custom_field.make_integer(field.config);
                break;
            case custom_field.TypeName.IntegerRange:
                item.type = custom_field.TypeName.IntegerRange;
                item.config = field.config;
                item.value = custom_field.make_integer_range(field.config);
                break;
            case custom_field.TypeName.Time:
                item.type = custom_field.TypeName.Time;
                item.config = field.config;
                item.value = custom_field.make_time(field.config);
                break;
            case custom_field.TypeName.TimeRange:
                item.type = custom_field.TypeName.TimeRange;
                item.config = field.config;
                item.value = custom_field.make_time_range(field.config);
                break;
        }

        custom_fields.push(item);
    }

    return custom_fields;
}

function blank_form(journal: JournalFull, entries_id: EntryId): EntryForm {
    let date = entries_id.type === 1 ? entries_id.date : new Date();

    return {
        id: null,
        uid: null,
        date,
        title: "",
        contents: "",
        tags: [],
        files: [],
        custom_fields: fields_from_journal(journal, new Set()),
    };
}

function sorter(a: EntryCustomFieldForm, b: EntryCustomFieldForm) {
    if (a.order > b.order) {
        return true;
    } else if (a.order < b.order) {
        return false;
    } else {
        return a.name > b.name;
    }
}

function entry_form(journal: JournalFull, entry: EntryForm): UIEntryForm {
    let known_fields: Set<number> = new Set();
    let custom_fields: EntryCustomFieldForm[] = [];

    for (let field of entry.custom_fields) {
        known_fields.add(field._id);
        custom_fields.push(field);
    }

    let missing = fields_from_journal(journal, known_fields);

    return {
        id: entry.id,
        uid: entry.uid,
        date: entry.date,
        title: entry.title ?? "",
        contents: entry.contents ?? "",
        tags: entry.tags,
        files: entry.files,
        custom_fields: merge_sorted(custom_fields, missing, sorter),
    };
}

interface EditEntryProps {
    journal: JournalFull,
    entries_id: EntryId,
    data: UIEntryForm,
    on_cancel: () => void,
    on_created: (data: EntryForm) => void,
    on_updated: (data: EntryForm) => void,
}

function EditEntry({
    journal,
    entries_id,
    data,
    on_cancel,
    on_created,
    on_updated,
}: EditEntryProps) {
    const form = useForm<UIEntryForm>({
        defaultValues: data,
    });

    const onSubmit: SubmitHandler<UIEntryForm> = async (data, event) => {
        if (data.id == null) {
            try {
                let result = await create_entry(journal.id, data);

                let successful: EntryFileForm[] = [];
                let failed: FailedFile[] = [];

                if (result.files.length !== 0) {
                    let uploaded = await parallel_uploads(
                        journal.id,
                        result.id!,
                        data.files,
                        result.files
                    );

                    successful = uploaded.successful;
                    failed = uploaded.failed;
                }

                (result as UIEntryForm).files = successful;

                form.reset(result);

                let total = result.files.length;

                for (let index = 0; index < failed.length; index += 1) {
                    form.setValue(`files.${index + total}`, failed[index]);
                }

                on_created(result);
            } catch(err) {
                console.error("failed to create entry:", err);
            }
        } else {
            try {
                let result = await update_entry(journal.id, data.id, data);

                let successful: EntryFileForm[] = [];
                let failed: FailedFile[] = [];

                if (result.files.length !== 0) {
                    let uploaded = await parallel_uploads(
                        journal.id,
                        result.id!,
                        data.files,
                        result.files
                    );

                    successful = uploaded.successful;
                    failed = uploaded.failed;
                }

                (result as UIEntryForm).files = successful;

                form.reset(result);

                let total = result.files.length;

                for (let index = 0; index < failed.length; index += 1) {
                    form.setValue(`files.${index + total}`, failed[index]);
                }

                on_updated(result);
            } catch(err) {
                console.error("failed to update entry:", err);
            }
        }
    };

    return <CenterPage>
        <FormProvider<UIEntryForm> {...form} children={
            <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
                <EditEntryHeader
                    journals_id={journal.id}
                    entries_id={entries_id}
                    loading={form.formState.isLoading || form.formState.isSubmitting}
                    on_cancel={on_cancel}
                />
                <FormField control={form.control} name="title" render={({field, formState}) => {
                    return <FormItem className="w-2/4">
                        <FormLabel>Title</FormLabel>
                        <FormControl>
                            <Input
                                ref={field.ref}
                                name={field.name}
                                disabled={formState.isLoading || formState.isSubmitting || field.disabled}
                                value={field.value}
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
                                value={field.value}
                                onBlur={field.onBlur}
                                onChange={field.onChange}
                            />
                        </FormControl>
                    </FormItem>
                }}/>
                <Separator/>
                <EditCustomFieldEntries />
                <Separator/>
                <EditTagEntry />
                <Separator/>
                <EditFileEntry journals_id={journal.id} entries_id={entries_id}/>
            </form>
        }/>
    </CenterPage>;
}

interface EditEntryHeaderProps {
    journals_id: number,
    entries_id: EntryId,
    loading: boolean,
    on_cancel: () => void,
}

function EditEntryHeader({journals_id, entries_id, loading, on_cancel}: EditEntryHeaderProps) {
    const navigate = useNavigate();
    const form = useFormContext<UIEntryForm>();
    let form_entries_id = form.getValues("id");

    return <div className="top-0 sticky flex flex-row flex-nowrap gap-x-4 bg-background border-b py-2">
        <Button type="button" variant="ghost" size="icon" onClick={() => navigate(-1)}>
            <ArrowLeft/>
        </Button>
        <FormField control={form.control} name="date" render={({field}) => {
            let date_value = typeof field.value === "string" ? naive_date_to_date(field.value) : field.value;

            return <FormItem>
                <Popover>
                    <PopoverTrigger asChild>
                        <FormControl>
                            <Button
                                variant="outline"
                                className={cn("w-[280px] justify-start text-left front-normal")}
                                disabled={entries_id.type === ENTRY_ID_DATE}
                            >
                                {format(date_value, "PPPP")}
                                <CalendarIcon className="mr-2 h-4 w-4"/>
                            </Button>
                        </FormControl>
                    </PopoverTrigger>
                    <PopoverContent className="w-auto p-0" align="start">
                        <Calendar
                            mode="single"
                            selected={date_value}
                            onDayBlur={field.onBlur}
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
        <div className="flex-1"/>
        <Button type="button" onClick={() => on_cancel()}>Cancel</Button>
        <Button type="submit" disabled={loading}>Save <Save/></Button>
        {form_entries_id != null ?
            <Button type="button" variant="destructive" disabled={loading} onClick={() => {
                delete_entry(journals_id, form_entries_id).then(() => {
                    navigate(-1);
                }).catch(err => {
                    console.error("failed to delete journal entry:", err);
                });
            }}>Delete<Trash/></Button>
            :
            null
        }
    </div>;
};

interface EditTagEntryProps {
}

function EditTagEntry({}: EditTagEntryProps) {
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
    on_selected: (files: FileList) => void,
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
                on_selected(e.target.files!);
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

    if (url == null) {
        return null;
    }

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

interface EditFileEntryProps {
    journals_id: number,
    entries_id: EntryId,
}

function EditFileEntry({journals_id, entries_id}: EditFileEntryProps) {
    const form = useFormContext<UIEntryForm>();
    const files = useFieldArray<UIEntryForm, "files">({
        control: form.control,
        name: "files"
    });

    return <div className="space-y-4">
        <div className="flex flex-row flex-nowrap gap-x-4 items-center">
            Files
            <AddFile on_selected={file_list => {
                for (let index = 0; index < file_list.length; index += 1) {
                    let file = file_list[index];

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
                    let mime = parse_mime(field.data.type) ?? default_mime();

                    download = <DownloadBtn src={field.data} name={field.name}/>;
                    player = <FilePreview mime_type={mime.type} data={field.data}/>;
                    break;
                }
                case "local": {
                    let mime = parse_mime(field.data.type) ?? default_mime();

                    player = <FilePreview mime_type={mime.type} data={field.data}/>;
                    break;
                }
                case "failed": {
                    status = <WarnButton message={"There was an error when sending the file to the server."}/>;

                    switch (field.original.type) {
                        case "local": {
                            let mime = parse_mime(field.original.data.type) ?? default_mime();

                            player = <FilePreview mime_type={mime.type} data={field.original.data}/>;
                            break;
                        }
                        case "in-memory": {
                            let mime = parse_mime(field.original.data.type) ?? default_mime();

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
