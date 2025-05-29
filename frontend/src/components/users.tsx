import { format } from "date-fns";
import { Plus, Save, Trash, Eye, EyeOff, RefreshCcw, Search, Check } from "lucide-react";
import { useState, } from "react";
import { useFieldArray, useFormContext, } from "react-hook-form";
import { Link, useParams, useNavigate } from "react-router-dom";

import { Button } from "@/components/ui/button";
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

export interface AttachedUser {
    users_id: number,
    username: string,
    added: string,
}

export function UserList<T>() {
    let form = useFormContext();
    let users = useFieldArray({
        control: form.control,
        name: "users",
    });

    let columns: ColumnDef<AttachedUser>[] = [
        {
            accessorKey: "username",
            header: "Username",
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
                        users.remove(row.index);
                    }}
                >
                    <Trash/>
                </Button>;
            }
        }
    ];

    return <div className="grow space-y-4 basis-1/2">
        <div className="flex flex-row flex-nowrap gap-x-4 items-center">
            <span>Users</span>
            <AddUsers on_added={new_user => {
                users.append(new_user);
            }}/>
        </div>
        <DataTable columns={columns} data={(users.fields as unknown) as AttachedUser[]}/>
    </div>
}


export interface AddUsersProps {
    on_added: (user: AttachedUser) => void,
}

export function AddUsers({on_added}: AddUsersProps) {
    let [loading, set_loading] = useState(false);
    let [data, set_data] = useState<UserPartial[]>([]);

    let columns: ColumnDef<UserPartial>[] = [
        {
            accessorKey: "username",
            header: "Username",
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
                            users_id: row.original.id,
                            username: row.original.username,
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
            let res = await fetch("/admin/users");

            if (res.status === 200) {
                let json = await res.json();

                set_data(json);
            }
        } catch (err) {
            console.error("failed to retrieve users", err);
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
                Add Users <Plus/>
            </Button>
        </SheetTrigger>
        <SheetContent>
            <SheetHeader>
                <SheetTitle>Add Users</SheetTitle>
                <SheetDescription>
                    Add users to the selected record
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
