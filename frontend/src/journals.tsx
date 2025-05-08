import { useState, useEffect, Fragment, PropsWithChildren } from "react";
import { useForm, useFieldArray, useFormContext, FormProvider, SubmitHandler,  } from "react-hook-form";
import { Routes, Route, Link, useParams, useNavigate } from "react-router-dom";
import { Plus, Save, Trash, RefreshCcw, Search, Check, Pencil, ArrowLeft } from "lucide-react";

import { send_json } from "@/net";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
    DropdownMenu,
    DropdownMenuTrigger,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuLabel,
} from "@/components/ui/dropdown-menu";
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
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import {
    Sheet,
    SheetContent,
    SheetDescription,
    SheetHeader,
    SheetTitle,
    SheetTrigger,
} from "@/components/ui/sheet";
import { Switch } from "@/components/ui/switch";
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
    custom_field,
} from "@/journals/api";
import {
    IntegerConfig,
    FloatConfig,
    TimeRangeConfig,
} from "@/journals/custom_fields";
import {
    JournalForm,
    blank_journal_form,
    blank_journal_custom_field_form,
} from "@/journals/forms";
import { Entry, Entries } from "@/journals/entries";
import { cn } from "@/utils";

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

function journal_to_form(journal: JournalFull) {
    let custom_fields = [];
    let peers = [];

    for (let field of journal.custom_fields) {
        custom_fields.push({
            _id: field.id,
            uid: field.uid,
            name: field.name,
            order: field.order,
            config: field.config,
            description: field.description ?? "",
        });
    }

    for (let peer of journal.peers) {
        peers.push(peer);
    }

    return {
        name: journal.name,
        description: journal.description ?? "",
        custom_fields,
        peers,
    };
}

interface JournalHeaderProps {
    journals_id: string,
    on_delete: () => void,
}

function JournalHeader({journals_id, on_delete}: JournalHeaderProps) {
    const navigate = useNavigate();
    const form = useFormContext<JournalForm>();

    return <div className="top-0 sticky flex flex-row flex-nowrap gap-x-4 bg-background border-b py-2">
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
                return blank_journal_form();
            }

            try {
                let result = await get_journal(journals_id);

                if (result != null) {
                    return journal_to_form(result);
                }
            } catch (err) {
                console.error("failed to retrieve journal", err);
            }

            return blank_journal_form();
        }
    });

    const create_journal = async (data: JournalForm) => {
        let custom_fields = [];
        let peers = [];

        for (let field of data.custom_fields) {
            let desc = field.description.trim();

            custom_fields.push({
                name: field.name,
                order: field.order,
                config: field.config,
                description: desc.length === 0 ? null : desc
            });
        }

        for (let peer of data.peers) {
            peers.push(peer.user_peers_id);
        }

        let desc = data.description.trim();

        let res = await send_json("POST", "/journals", {
            name: data.name,
            description: desc.length === 0 ? null : desc,
            custom_fields,
            peers,
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
        let custom_fields = [];
        let peers = [];

        for (let field of data.custom_fields) {
            let desc = field.description.trim();
            let obj = {
                name: field.name,
                order: field.order,
                config: field.config,
                description: desc.length === 0 ? null : desc
            };

            if (field._id != null) {
                obj["id"] = field._id;
            }

            custom_fields.push(obj);
        }

        for (let peer of data.peers) {
            peers.push(peer.user_peers_id);
        }

        let description = data.description.trim();

        let res = await send_json("PATCH", `/journals/${journals_id}`, {
            name: data.name,
            description: description.length === 0 ? null : description,
            custom_fields,
            peers,
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

    const on_delete = () => {};

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
        return <CenterPage className="pt-2">
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
                <Separator />
                <PeersList />
                <Separator />
                <CustomFieldList />
            </form>
        }/>
    </CenterPage>;
}

function PeersList() {
    const form = useFormContext<JournalForm>();
    const peers = useFieldArray<JournalForm, "peers">({
        control: form.control,
        name: "peers"
    });

    let apply_flex = peers.fields.length > 1;
    let include_spacer = peers.fields.length % 2 !== 0;

    let peer_eles = peers.fields.map((peer, index) => (
        <div
            key={peer.id}
            className="flex flex-row items-center rounded-lg border p-4 basis-[45%] grow"
        >
            <span className="grow">{peer.name}</span>
            <Button type="button" variant="destructive" size="icon" onClick={() => {
                peers.remove(index);
            }}>
                <Trash/>
            </Button>
        </div>
    ));

    if (peers.fields.length % 2 !== 0) {
        peer_eles.push(<div key="spacer" className="basis-[45%] grow"/>);
    }

    return <div className="space-y-4">
        <div className="flex flex-row flex-nowrap gap-x-4 items-center">
            Peers
            <AddPeer on_added={peer => {
                peers.append({
                    user_peers_id: peer.id,
                    name: peer.name,
                    synced: null,
                });
            }}/>
        </div>
        {peers.fields.length !== 0 ?
            <div className="flex flex-row flex-wrap gap-2">
                {peer_eles}
            </div>
            :
            null
        }
    </div>;
}

interface AddPeerProps {
    on_added: (peer: UserPeerPartial) => void
}

interface UserPeerPartial {
    id: number,
    name: string
}

function AddPeer({on_added}: AddPeerProps) {
    const [loading, set_loading] = useState(false);
    const [data, set_data] = useState<UserPeerPartial[]>([]);

    const columns: ColumnDef<UserPeerPartial>[] = [
        {
            accessorKey: "name",
            header: "Name",
        },
        {
            id: "selector",
            cell: ({ row }) => (
                <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    onClick={() => on_added(row.original)}
                >
                    <Plus/>
                </Button>
            )
        }
    ];

    const retrieve = async () => {
        if (loading) {
            return;
        }

        set_loading(true);

        try {
            let res = await fetch("/peers");

            if (res.status === 200) {
                let json = await res.json();

                set_data(json);
            }
        } catch (err) {
            console.error("failed to retrieve peers", err);
        }

        set_loading(false);
    };

    return <Sheet onOpenChange={value => {
        if (value) {
            retrieve();
        }
    }}>
        <SheetTrigger asChild>
            <Button type="button" variant="secondary">
                <Plus/>Add Peer
            </Button>
        </SheetTrigger>
        <SheetContent>
            <SheetHeader>
                <SheetTitle>Add Peer</SheetTitle>
                <SheetDescription>
                    Add remote peers synchronize the journal to.
                </SheetDescription>
            </SheetHeader>
            <DataTable columns={columns} data={data}/>
        </SheetContent>
    </Sheet>
}

interface CustomFieldListProps {
}

function CustomFieldList({}: CustomFieldListProps) {
    const form = useFormContext<JournalForm>();
    const custom_fields = useFieldArray<JournalForm, "custom_fields">({
        control: form.control,
        name: "custom_fields",
    });

    return <div className="space-y-4">
        <div className="flex flex-row flex-nowrap gap-x-4 items-center">
            Custom Fields
            <DropdownMenu>
                <DropdownMenuTrigger asChild>
                    <Button type="button" variant="secondary">
                        <Plus/>Add Field
                    </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent>
                    <DropdownMenuItem onSelect={ev => {
                        custom_fields.append(blank_journal_custom_field_form(custom_field.TypeName.Integer));
                    }}>Integer</DropdownMenuItem>
                    <DropdownMenuItem onSelect={ev => {
                        custom_fields.append(blank_journal_custom_field_form(custom_field.TypeName.IntegerRange));
                    }}>Integer Range</DropdownMenuItem>
                    <DropdownMenuItem onSelect={ev => {
                        custom_fields.append(blank_journal_custom_field_form(custom_field.TypeName.Float));
                    }}>Float</DropdownMenuItem>
                    <DropdownMenuItem onSelect={ev => {
                        custom_fields.append(blank_journal_custom_field_form(custom_field.TypeName.FloatRange));
                    }}>Float Range</DropdownMenuItem>
                    <DropdownMenuItem onSelect={ev => {
                        custom_fields.append(blank_journal_custom_field_form(custom_field.TypeName.TimeRange));
                    }}>Time Range</DropdownMenuItem>
                </DropdownMenuContent>
            </DropdownMenu>
        </div>
        {custom_fields.fields.map((field, index) => {
            let type_ui = null;
            let type_desc = null;

            switch (field.config.type) {
            case custom_field.TypeName.Integer:
                type_ui = <IntegerConfig id={field.id} config={field.config} index={index}/>;
                type_desc = <FieldDesc title="Integer">
                    Single whole number input that can have an optional minimum and maximum value.
                </FieldDesc>;
                break;
            case custom_field.TypeName.IntegerRange:
                type_ui = <IntegerConfig id={field.id} config={field.config} index={index}/>;
                type_desc = <FieldDesc title="Integer Range">
                    Whole number input that can specify a range between a low and high value with an optional minimum and maximum value.
                </FieldDesc>;
                break;
            case custom_field.TypeName.Float:
                type_ui = <FloatConfig id={field.id} config={field.config} index={index}/>;
                type_desc = <FieldDesc title="Float">
                    Single decimal number input that can have an optional minimum and maximum value.
                    Can also specify the precision of the value and the step at which to increase that value by.
                </FieldDesc>;
                break;
            case custom_field.TypeName.FloatRange:
                type_ui = <FloatConfig id={field.id} config={field.config} index={index}/>;
                type_desc = <FieldDesc title="Float Range">
                    Decimal number input that can specify a range between a low and high value with an optional minimum and maximum value.
                    Can also specify the precision of the value and the step at which to increase that value by.
                </FieldDesc>;
                break;
            case custom_field.TypeName.Time:
                type_desc = <FieldDesc title="Time">
                    under consideration
                </FieldDesc>;
                break;
            case custom_field.TypeName.TimeRange:
                type_ui = <TimeRangeConfig id={field.id} config={field.config} index={index}/>;
                type_desc = <FieldDesc title="Time Range">
                    Time input that can specify a range between a low and high value.
                </FieldDesc>;
                break;
            }

            return <div key={field.id} className="rounded-lg border">
                <div className="flex flex-row flex-nowrap gap-x-4 p-4">
                    <FormField control={form.control} name={`custom_fields.${index}.name`} render={({field: name_field}) => {
                        return <FormItem className="w-1/2">
                            <FormControl>
                                <Input type="text" placeholder="Custom Field Name" {...name_field}/>
                            </FormControl>
                        </FormItem>
                    }}/>
                    <Button type="button" variant="destructive" size="icon" onClick={() => {
                        custom_fields.remove(index);
                    }}><Trash/></Button>
                </div>
                <Separator/>
                <div className="p-4 space-y-4">
                    {type_desc}
                    <FormField control={form.control} name={`custom_fields.${index}.order`} render={({field: order_field}) => {
                        return <FormItem>
                            <FormLabel>Order</FormLabel>
                            <FormControl>
                                <Input
                                    ref={order_field.ref}
                                    disabled={order_field.disabled}
                                    name={order_field.name}
                                    value={order_field.value}
                                    className="w-1/4"
                                    type="number"
                                    min="0"
                                    max="100"
                                    onBlur={order_field.onBlur}
                                    onChange={ev => {
                                        order_field.onChange(parseInt(ev.target.value, 10));
                                    }}
                                />
                            </FormControl>
                            <FormDescription>
                                This will determine the display order of the fields when creating a new entry.
                                Higher order values have higher priority when being displayed.
                                If multiple fields have the same value then the name of the field will determine the sort order.
                            </FormDescription>
                        </FormItem>
                    }}/>
                    <FormField control={form.control} name={`custom_fields.${index}.description`} render={({field: desc_field}) => {
                        return <FormItem className="w-3/4">
                            <FormLabel>Description</FormLabel>
                            <FormControl>
                                <Textarea type="text" {...desc_field}/>
                            </FormControl>
                        </FormItem>
                    }}/>
                    {type_ui}
                </div>
            </div>;
        })}
    </div>;
}

interface FieldDescProps {
    title: string,
}

type FieldDescChildProps = PropsWithChildren<FieldDescProps>;

function FieldDesc({title, children}: FieldDescChildProps) {
    return <div>
        <span className="text-sm font-medium inline">{title} </span>
        <p className="text-sm text-muted-foreground inline">
            {children}
        </p>
    </div>;
}
