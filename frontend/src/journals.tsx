import { useState, useEffect, Fragment } from "react";
import { useForm, useFieldArray, useFormContext, FormProvider, SubmitHandler,  } from "react-hook-form";
import { Routes, Route, Link, useParams, useNavigate } from "react-router-dom";
import { Plus, Save, Trash, RefreshCcw, Search, Check, Pencil, ArrowLeft } from "lucide-react";

import { Button } from "@/components/ui/button";
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
import { Separator } from "@/components/ui/separator";
import {
    Sheet,
    SheetContent,
    SheetDescription,
    SheetHeader,
    SheetTitle,
    SheetTrigger,
} from "@/components/ui/sheet";
import {
    DataTable,
    ColumnDef,
} from "@/components/ui/table";
import { Textarea } from "@/components/ui/textarea";
import {
    JournalPartial,
    JournalFull,
    get_journals,
    get_journal,
} from "@/journals/api";
import { Entry, Entries } from "@/journals/entries";

export function JournalRoutes() {
    return <Routes>
        <Route index element={<JournalsIndex />}/>
        <Route path="/:journals_id" element={<Journal />}/>
        <Route path="/:journals_id/entries" element={<Entries />}/>
        <Route path="/:journals_id/entries/:entries_id" element={<Entry />}/>
    </Routes>;
}

function JournalsIndex() {
    return <CenterPage className="flex items-center justify-center h-full">
        <div className="w-1/2 flex flex-col flex-nowrap items-center">
            <h2 className="text-2xl">Nothing to see here</h2>
            <p>Select a journal on the sidebar to view its entries</p>
        </div>
    </CenterPage>;
}

interface JournalForm {
    name: string,
    description: string
}

function blank_form() {
    return {
        name: "",
        description: "",
    };
}

function journal_to_form(journal: JournalFull) {
    return {
        name: journal.name,
        description: journal.description ?? "",
    };
}

interface JournalHeaderProps {
    journals_id: string,
    on_delete: () => void,
}

function JournalHeader({journals_id, on_delete}: JournalHeaderProps) {
    const navigate = useNavigate();
    const form = useFormContext<JournalForm>();

    return <div className="top-0 sticky flex flex-row flex-nowrap gap-x-4">
        <Link to="/journals">
            <Button type="button" variant="ghost" size="icon">
                <ArrowLeft/>
            </Button>
        </Link>
        <FormField control={form.control} name="name" render={({ field }) => {
            return <FormItem>
                <FormControl>
                    <Input type="text" placeholder="Name" {...field}/>
                </FormControl>
            </FormItem>
        }}/>
        <Button type="submit">
            Save<Save/>
        </Button>
        {journals_id !== "new" ?
            <Button
                type="button"
                variant="destructive"
                disabled
                onClick={() => {
                    on_delete();
                }}
            >
                Delete
                <Trash/>
            </Button>
            :
            null
        }
    </div>;
}

function Journal() {
    const { journals_id } = useParams();
    const navigate = useNavigate();

    const form = useForm<JournalForm>({
        defaultValues: async () => {
            if (journals_id === "new") {
                return blank_form();
            }

            try {
                let result = await get_journal(journals_id);

                if (result != null) {
                    return journal_to_form(result);
                }
            } catch (err) {
                console.error("failed to retrieve journal", err);
            }

            return blank_form();
        }
    });

    const create_journal = async (data: JournalForm) => {
        let description = data.description.trim();

        let body = JSON.stringify({
            name: data.name,
            description: description.length === 0 ? null : description
        });

        let res = await fetch("/journals", {
            method: "POST",
            headers: {
                "content-type": "application/json",
                "content-length": body.length.toString(10)
            },
            body
        });

        switch (res.status) {
        case 200:
            return await res.json();
        case 400:
            let json = await res.json();

            console.error("failed to create journal", json);
            break;
        case 403:
            console.error("you do not have permission to create journals");
            break;
        default:
            console.warn("unhandled response status code");
            break;
        }

        return null;
    };

    const update_journal = async (data: JournalForm) => {
        let description = data.description.trim();

        let body = JSON.stringify({
            name: data.name,
            description: description.length === 0 ? null : description
        });

        let res = await fetch(`/journals/${journals_id}`, {
            method: "PATCH",
            headers: {
                "content-type": "application/json",
                "content-length": body.length.toString(10),
            },
            body
        });

        switch (res.status) {
        case 200:
            return true;
        case 400:
            let json = await res.json();

            console.error("failed to update journal", json);
            break;
        case 403:
            console.error("you do not have permission to update journals");
            break;
        case 404:
            console.error("journal not found");
            break;
        default:
            console.warn("unhandled response status code");
            break;
        }

        return false;
    };

    const on_delete = () => {
        
    };

    const on_submit: SubmitHandler<JournalForm> = async (data, event) => {
        if (journals_id === "new") {
            try {
                let created = await create_journal(data);

                if (created == null) {
                    return;
                }

                form.reset(journal_to_form(created));

                navigate(`/journals/${created.id}`);
            } catch (err) {
                console.error("error when creating new journal", err);
            }
        } else {
            try {
                if (await update_journal(data)) {
                    form.reset(data);
                }
            } catch (err) {
                console.error("error when updating journal", err);
            }
        }
    };

    if (form.formState.isLoading) {
        return <CenterPage>
            loading journal
        </CenterPage>;
    }

    return <CenterPage>
        <FormProvider<JournalForm> {...form} children={
            <form onSubmit={form.handleSubmit(on_submit)} className="space-y-4">
                <JournalHeader journals_id={journals_id} on_delete={on_delete}/>
                <FormField control={form.control} name="description" render={({ field }) => {
                    return <FormItem className="w-1/2">
                        <FormLabel>Description</FormLabel>
                        <FormControl>
                            <Textarea type="text" {...field}/>
                        </FormControl>
                    </FormItem>
                }}/>
            </form>
        }/>
    </CenterPage>;
}
