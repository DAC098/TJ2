import { format } from "date-fns";
import { useState, useEffect, JSX } from "react";
import { Link, useParams, useNavigate } from "react-router-dom";
import { Plus, CalendarIcon, Trash, Save, ArrowLeft } from "lucide-react";
import { useForm, useFieldArray, useFormContext, FormProvider, SubmitHandler,  } from "react-hook-form";

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
import {
    DataTable,
    ColumnDef,
} from "@/components/ui/table";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/utils";
import TagEntry from "@/journals/TagEntry";
import FileEntry from "@/journals/FileEntry";
import {
    EntryPartial,
    Entry,
    EntryFileForm,
    EntryForm,
    EntryTagForm,
    get_date,
    blank_form,
    entry_to_form,
    retrieve_entry,
    create_entry,
    update_entry,
    delete_entry,
    upload_data,
} from "@/journals/api";

async function retrieve_entries(journals_id: string) {
    let res = await fetch(`/journals/${journals_id}/entries`);

    if (res.status !== 200) {
        return null;
    }

    return await res.json() as EntryPartial[];
}

export function Entries() {
    const { journals_id } = useParams();

    let [loading, setLoading] = useState(false);
    let [entries, setEntries] = useState<EntryPartial[]>([]);

    useEffect(() => {
        setLoading(true);

        retrieve_entries(journals_id).then(json => {
            setEntries(() => {
                return json;
            });
        }).catch(err => {
            console.error("failed to retrieve entries:", err);
        }).finally(() => {
            setLoading(false);
        });
    }, [journals_id]);

    const columns: ColumnDef<EntryPartial>[] = [
        {
            accessorKey: "date",
            header: "Date",
            cell: ({ row }) => {
                return <Link to={`/journals/${journals_id}/entries/${row.original.id}`}>{row.original.date}</Link>;
            }
        },
        {
            accessorKey: "title",
            header: "Title",
        },
        {
            accessorKey: "tags",
            header: "Tags",
            cell: ({ row }) => {
                let list = [];

                for (let tag in row.original.tags) {
                    list.push(<span key={tag}>{tag}</span>);
                }

                return <>{list}</>;
            }
        },
        {
            accessorKey: "mod",
            header: "Mod",
            cell: ({ row }) => {
                return row.original.updated != null ? row.original.updated : row.original.created;
            }
        }
    ];

    return <CenterPage>
        <div className="flex flex-row flex-nowrap gap-x-4">
            <Link to={`/journals/${journals_id}/entries/new`}>
                <Button type="button">New Entry<Plus/></Button>
            </Link>
        </div>
        <DataTable columns={columns} data={entries}/>
    </CenterPage>
};

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

    return <div className="top-0 sticky flex flex-row flex-nowrap gap-x-4">
        <Link to={`/journals/${journals_id}/entries`}>
            <Button type="button" variant="ghost" size="icon">
                <ArrowLeft/>
            </Button>
        </Link>
        <FormField control={form.control} name="date" render={({field}) => {
            return <FormItem>
                <Popover>
                    <PopoverTrigger asChild>
                        <FormControl>
                            <Button
                                variant="outline"
                                className={cn("w-[280px] justify-start text-left front-normal")}
                                disabled={loading}
                            >
                                {format(field.value, "PPP")}
                                <CalendarIcon className="mr-2 h-4 w-4"/>
                            </Button>
                        </FormControl>
                    </PopoverTrigger>
                    <PopoverContent className="w-auto p-0" aligh="start">
                        <Calendar
                            mode="single"
                            selected={field.value}
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
    entry_form: EntryForm,
    entry: Entry,
): Promise<EntryFileForm[]> {
    let mapped = create_file_map(entry_form.files);
    let to_upload = [];
    let uploaders = [];

    for (let file_entry of entry.files) {
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

                console.log("uploader:", index, "sending file:", file_entry.id);

                try {
                    let successful = await upload_data(
                        entry.journals_id,
                        entry.id,
                        file_entry,
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
            let rtn = blank_form();

            if (entries_id == null || entries_id === "new") {
                return rtn;
            }

            try {
                let entry = await retrieve_entry(journals_id, entries_id);

                rtn = entry_to_form(entry);
            } catch (err) {
                console.log("failed to retrieve entry", err);
            }

            return rtn;
        },
        disabled: false,
    });

    const create_and_upload = async (entry: EntryForm): Promise<[Entry, EntryFileForm[]]> => {
        let result = await create_entry(journals_id, entry);

        if (result.files.length === 0) {
            return [result, []];
        }

        let failed_uploads = await parallel_uploads(entry, result);

        return [result, failed_uploads];
    };

    const update_and_upload = async (entry: EntryForm): Promise<[Entry, EntryFileForm[]]> => {
        let result = await update_entry(journals_id, entries_id, entry);

        if (result.files.length === 0) {
            return [result, []];
        }

        let failed_uploads = await parallel_uploads(entry, result);

        return [result, failed_uploads];
    };

    const onSubmit: SubmitHandler<EntryForm> = async (data, event) => {
        if (entries_id == null || entries_id == "new") {
            try {
                let [result, failed] = await create_and_upload(data);

                console.log("created entry:", result, failed);

                if (failed.length === 0) {
                    form.reset(entry_to_form(result));

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
                    form.reset(entry_to_form(result));
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
                <FormField control={form.control} name="title" render={({field}) => {
                    return <FormItem className="w-2/4">
                        <FormLabel>Title</FormLabel>
                        <FormControl>
                            <Input disabled={form.formState.isLoading || form.formState.isSubmitting} {...field}/>
                        </FormControl>
                    </FormItem>
                }}/>
                <FormField control={form.control} name="contents" render={({field}) => {
                    return <FormItem className="w-3/4">
                        <FormLabel>Contents</FormLabel>
                        <FormControl>
                            <Textarea disabled={form.formState.isLoading || form.formState.isSubmitting} {...field}/>
                        </FormControl>
                    </FormItem>
                }}/>
                <TagEntry loading={form.formState.isLoading || form.formState.isSubmitting}/>
                <FileEntry loading={form.formState.isLoading || form.formState.isSubmitting} entries_id={entries_id}/>
            </form>
        }/>
    </div>
};
