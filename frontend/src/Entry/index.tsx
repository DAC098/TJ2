import { useState, useEffect, JSX } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { format } from "date-fns";
import { useForm, useFieldArray, useFormContext, FormProvider, SubmitHandler,  } from "react-hook-form";
import { CalendarIcon, Trash, Save } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Calendar } from "@/components/ui/calendar";
import {
    FormControl,
    FormDescription,
    FormField,
    FormItem,
    FormLabel,
    FormMessage,
} from "@/components/ui/form";
import {
    Popover,
    PopoverContent,
    PopoverTrigger,
} from "@/components/ui/popover";
import { cn } from "@/utils";
import TagEntry from "@/Entry/TagEntry";
import FileEntry from "@/Entry/FileEntry";
import {
    JournalEntry,
    JournalTag,
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
} from "@/journal";

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
    entry_date: string,
}

const EntryHeader = ({entry_date}: EntryHeaderProps) => {
    const navigate = useNavigate();
    const form = useFormContext<EntryForm>();

    return <div className="top-0 sticky flex flex-row flex-nowrap gap-x-4">
        <FormField control={form.control} name="date" render={({field}) => {
            return <FormItem>
                <Popover>
                    <PopoverTrigger asChild>
                        <FormControl>
                            <Button
                                variant="outline"
                                className={cn("w-[280px] justify-start text-left front-normal")}
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
        <Button type="submit">Save<Save/></Button>
        {entry_date != null && entry_date !== "new" ?
            <Button type="button" variant="destructive" onClick={() => {
                delete_entry(entry_date).then(() => {
                    console.log("deleted entry");

                    navigate("/entries");
                }).catch(err => {
                    console.error("failed to delete journal entry:", err);
                });
            }}>Delete<Trash/></Button>
            :
            null
        }
    </div>;
};

const Entry = () => {
    const { entry_date } = useParams();
    const navigate = useNavigate();

    const form = useForm<EntryForm>({
        defaultValues: blank_form()
    });
    const tags = useFieldArray<EntryForm, "tags">({
        control: form.control,
        name: "tags"
    });

    let [loading, setLoading] = useState(true);

    const create_and_upload = async (entry: EntryForm): Promise<[boolean, JournalEntry]> => {
        let mapped = {};
        let promise = create_entry(entry);

        if (entry.files.length !== 0) {
            for (let file of entry.files) {
                if (file.type == "server") {
                    continue;
                }

                mapped[file.key] = file;
            }
        }

        let failed = false;
        let result = await promise;

        console.log("created entry:", result);

        if (result.files.length !== 0 ) {
            let promises = [];

            for (let file_entry of result.files) {
                let ref = mapped[file_entry.attached.key];

                if (ref == null) {
                    console.log("file key not known to client", file_entry.attached.key);

                    continue;
                }

                let prom = upload_data(result.date, file_entry, ref)
                    .then(success => {
                        if (success) {
                            console.log("uploaded file", file_entry.id);
                        } else {
                            failed = true;
                        }
                    });

                promises.push(prom);
            }

            let prom_results = await Promise.allSettled(promises);

            for (let prom of prom_results) {
                switch (prom.status) {
                case "fulfilled":
                    console.log("file uploaded", prom.value);

                    break;
                case "rejected":
                    console.log("file failed", prom.reason);

                    failed = true;

                    break;
                }
            }
        }

        return [failed, result];
    };

    const onSubmit: SubmitHandler<EntryForm> = async (data, event) => {
        console.log(data);

        if (entry_date == null || entry_date == "new") {
            try {
                let [failed, result] = await create_and_upload(data);

                if (!failed) {
                    form.reset(entry_to_form(result));

                    navigate(`/entries/${result.date}`);
                }
            } catch(err) {
                console.error("failed to create entry:", err);
            }
        } else {
            try {
                let result = await update_entry(entry_date, data);

                console.log("updated entry:", result);
            } catch(err) {
                console.error("failed to update entry:", err);
            }
        }
    };

    useEffect(() => {
        console.log("entry date:", entry_date);

        let form_date = form.getValues("date");

        if (form_date == entry_date) {
            console.log("form date is same as entry, assume entry was just created");
        }

        if (entry_date == null || entry_date == "new") {
            console.log("resetting to blank");

            form.reset(blank_form());

            return;
        }

        retrieve_entry(entry_date).then(entry => {
            console.log("resetting to entry:", entry);

            form.reset(entry_to_form(entry));
        }).catch(err => {
            console.error("failed to retrieve entry:", err);
        });
    }, [entry_date]);

    return <div className="max-w-3xl mx-auto my-auto">
        <FormProvider<EntryForm> {...form} children={
            <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
                <EntryHeader entry_date={entry_date}/>
                <FormField control={form.control} name="title" render={({field}) => {
                    return <FormItem className="w-2/4">
                        <FormLabel>Title</FormLabel>
                        <FormControl>
                            <Input {...field}/>
                        </FormControl>
                    </FormItem>
                }}/>
                <FormField control={form.control} name="contents" render={({field}) => {
                    return <FormItem className="w-3/4">
                        <FormLabel>Contents</FormLabel>
                        <FormControl>
                            <Textarea {...field}/>
                        </FormControl>
                    </FormItem>
                }}/>
                <TagEntry />
                <FileEntry entry_date={entry_date}/>
            </form>
        }/>
    </div>
};

export default Entry;
