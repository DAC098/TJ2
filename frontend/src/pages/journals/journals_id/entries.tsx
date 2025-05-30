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
import { parse_mime, default_mime } from "@/parse";

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
