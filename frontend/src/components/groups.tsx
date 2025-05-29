import { format } from "date-fns";
import { Plus, Save, Trash, Eye, EyeOff, RefreshCcw, Search, Check } from "lucide-react";
import { useState, useEffect } from "react";
import { useForm, useFieldArray, useFormContext, FormProvider, SubmitHandler } from "react-hook-form";
import { Link, useParams, useNavigate } from "react-router-dom";

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

export interface AttachedGroup {
    groups_id: number,
    name: string,
    added: string,
}

export function GroupList() {
    let form = useFormContext();
    let groups = useFieldArray({
        control: form.control,
        name: "groups",
    });

    let columns: ColumnDef<AttachedGroup>[] = [
        {
            accessorKey: "name",
            header: "Name",
        },
        {
            header: "Added",
            cell: ({ row }) => {
                let date = new Date(row.original.added);

                return format(date, "yyyy/MM/dd");
            }
        },
        {
            id: "drop",
            cell: ({ row }) => {
                return <Button
                    type="button"
                    variant="destructive"
                    size="icon"
                    onClick={() => {
                        groups.remove(row.index);
                    }}
                >
                    <Trash/>
                </Button>;
            }
        }
    ];

    return <div className="grow space-y-4 basis-1/2">
        <div className="flex flex-row flex-nowrap gap-x-4 items-center">
            <span>Groups</span>
            <AddGroups on_added={new_group => {
                groups.append(new_group);
            }}/>
        </div>
        <DataTable columns={columns} data={(groups.fields as unknown) as AttachedGroup[]}/>
    </div>
}

interface AddGroupsProps {
    on_added: (group: AttachedGroup) => void,
}

function AddGroups({on_added}: AddGroupsProps) {
    let [loading, set_loading] = useState(false);
    let [data, set_data] = useState<GroupPartial[]>([]);

    let columns: ColumnDef<GroupPartial>[] = [
        {
            accessorKey: "name",
            header: "Name",
        },
        {
            id: "selector",
            cell: ({ row }) => {
                return <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    onClick={() => {
                        on_added({
                            groups_id: row.original.id,
                            name: row.original.name,
                            added: (new Date()).toJSON(),
                        });
                    }}
                >
                    <Plus/>
                </Button>;
            }
        }
    ];

    const retrieve = async () => {
        if (loading) {
            return;
        }

        set_loading(true);

        try {
            let res = await fetch("/admin/groups");

            if (res.status === 200) {
                let json = await res.json();

                set_data(json);
            }
        } catch (err) {
            console.error("failed to retrieve groups", err);
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
                Add Groups <Plus/>
            </Button>
        </SheetTrigger>
        <SheetContent>
            <SheetHeader>
                <SheetTitle>Add Groups</SheetTitle>
                <SheetDescription>
                    Add groups to the selected record
                </SheetDescription>
            </SheetHeader>
            <div className="space-y-4 mt-4">
                <div className="flex flex-row flex-nowrap gap-x-4 items-center">
                    <div className="w-full relative">
                        <Input type="text" placeholder="Search" className="pr-10" disabled={loading}/>
                        <Button
                            type="button"
                            variant="ghost"
                            size="icon"
                            className="absolute right-0 top-0"
                            disabled={loading}
                            onClick={() => {
                                retrieve();
                            }}
                        >
                            <Search/>
                        </Button>
                    </div>
                    <Button type="button" variant="ghost" size="icon" onClick={() => {}}>
                        <RefreshCcw/>
                    </Button>
                </div>
                <DataTable columns={columns} data={data}/>
            </div>
        </SheetContent>
    </Sheet>
}
