import { format, formatDistanceToNow } from "date-fns";
import { Plus, Trash } from "lucide-react";
import { useRef, useState, useEffect, useMemo, JSX } from "react";
import { useForm, useFormContext, FormProvider, SubmitHandler, Form } from "react-hook-form";
import { Link, useParams, useNavigate } from "react-router-dom";

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
    DialogTrigger,
} from "@/components/ui/dialog";
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
    DataTable,
    ColumnDef,
} from "@/components/ui/table";
import { send_to_clipboard } from "@/utils";

interface UserKeys {
    public_key: string,
    clients: UserClient[],
    peers: UserPeer[],
}

interface UserClient {
    id: number,
    name: string,
    public_key: string,
    created: string,
    updated: string | null
}

interface UserPeer {
    id: number,
    name: string,
    public_key: string,
    addr: string,
    port: number,
    secure: boolean,
    ssc: boolean,
    created: string,
    updated: string,
}

export function PeerClient() {
    const [loading, set_loading] = useState(false);
    const [user_keys, set_user_keys] = useState<UserKeys>({
        public_key: "HERE",
        clients: [],
        peers: []
    });

    async function retrieve_keys() {
        set_loading(true);

        try {
            let res = await fetch("/settings/peer_client");

            if (res.status === 200) {
                let json = await res.json();

                set_user_keys(json);
            } else {
                console.warn("non 200 status");
            }
        } catch (err) {
            console.error("failed to request user keys");
        }

        set_loading(false);
    }

    useEffect(() => {
        retrieve_keys();
    }, []);

    return <CenterPage className="pt-4 max-w-xl space-y-4">
        <div className="space-y-4">
            <div className="space-y-2">
                <h2 className="text-xl">Local Public Key</h2>
                <p className="text-sm">
                    This is the public key that you will provide to other servers
                    to allow this server to act as a client on your behalf.
                </p>
            </div>
            <div
                className="underline cursor-pointer"
                onClick={() => send_to_clipboard(user_keys.public_key).then(() => {
                    console.log("wrote to clipboard");
                }).catch(err => {
                    console.error("failed writing to clipboard", err);
                })}
            >
                public key: {user_keys.public_key}
            </div>
        </div>
        <Separator />
        <div className="space-y-4">
            <div className="flex flex-row flex-nowrap items-center">
                <div className="space-y-2 grow">
                    <h2 className="text-xl">Client Keys</h2>
                    <p className="text-sm">
                        This is the list of keys that are allowed to access this account.
                    </p>
                </div>
                <AddClient on_added={(client) => {
                    set_user_keys(value => {
                        let clients = [...value.clients, client].sort((a, b) => {
                            return a.name.localeCompare(b.name);
                        });

                        return {
                            ...value,
                            clients,
                        };
                    });
                }}/>
            </div>
            <ClientList clients={user_keys.clients} on_delete={(id) => {
                set_user_keys(value => {
                    let filtered = value.clients.filter(c => c.id !== id);

                    return {
                        ...value,
                        clients: filtered,
                    };
                });
            }}/>
        </div>
        <Separator />
        <div className="space-y-4">
            <div className="flex flex-row flex-nowrap items-center">
                <div className="space-y-2 grow">
                    <h2 className="text-xl">Peer Keys</h2>
                    <p className="text-sm">
                        This is a list of keys that are from peer servers.
                    </p>
                </div>
                <AddPeer on_added={(peer) => {
                    set_user_keys(value => {
                        let peers = [...value.peers, peer].sort((a, b) => {
                            return a.name.localeCompare(b.name);
                        });

                        return {
                            ...value,
                            peers,
                        }
                    });
                }}/>
            </div>
            <PeerList peers={user_keys.peers} on_delete={(id) => {
                set_user_keys(value => {
                    let filtered = value.peers.filter(p => p.id !== id);

                    return {
                        ...value,
                        peers: filtered,
                    };
                });
            }}/>
        </div>
    </CenterPage>;
}

interface ClientListProps {
    clients: UserClient[]
    on_delete: (id: number) => void,
}

function ClientList({clients, on_delete}: ClientListProps) {
    return <>
        {clients.map((client) => <ClientListItem
            key={client.id}
            client={client}
            on_delete={() => {
                on_delete(client.id);
            }}
        />)}
    </>;
}

interface ClientListItemProps {
    client: UserClient,
    on_delete: () => void,
}

function ClientListItem({client, on_delete}: ClientListItemProps) {
    const [loading, set_loading] = useState(false);

    const delete_client = async () => {
        set_loading(true);

        try {
            let body = JSON.stringify({
                type: "Client",
                id: client.id,
            });
            let res = await fetch("/settings/peer_client", {
                method: "DELETE",
                headers: {
                    "content-type": "application/json",
                    "content-length": body.length.toString(10),
                },
                body
            });

            if (res.status === 200) {
                console.log("peer deleted");

                on_delete();
            } else {
                let json = await res.json();

                console.warn("failed to delete client", json);
            }
        } catch (err) {
            console.error("error when trying to delete client item", err);
        }

        set_loading(false);
    };

    let create_date = new Date(client.created);
    let create_distance = formatDistanceToNow(create_date, {
        addSuffix: true,
        includeSeconds: true,
    });
    let update_ele = null;

    if (client.updated != null) {
        let update_date = new Date(client.updated);
        let update_distance = formatDistanceToNow(update_date, {
            addSuffix: true,
            includeSeconds: true,
        });

        update_ele = <span title={update_date.toString()}>Modified: {update_distance}</span>;
    }

    return <div className="rounded-lg border p-4 space-y-4">
        <div className="flex flex-row items-center justify-between">
            <h3 className="text-lg grow">{client.name}</h3>
            <Button
                type="button"
                size="icon"
                variant="destructive"
                disabled={loading}
                onClick={() => delete_client()}
            >
                <Trash/>
            </Button>
        </div>
        <div className="flex flex-col gap-y-1">
            <span>Public Key: {client.public_key}</span>
            <span title={create_date.toString()}>Created: {create_distance}</span>
            {update_ele}
        </div>
    </div>;
}

interface NewClient {
    name: string,
    public_key: string,
}

interface AddClientProps {
    on_added: (client: UserClient) => void,
}

function AddClient({on_added}: AddClientProps) {
    const [is_open, set_is_open] = useState(false);

    const form = useForm<NewClient>({
        defaultValues: {
            name: "",
            public_key: "",
        }
    });

    const on_submit: SubmitHandler<NewClient> = async (data, event) => {
        try {
            let body = JSON.stringify({
                type: "Client",
                ...data,
            });
            let res = await fetch("/settings/peer_client", {
                method: "POST",
                headers: {
                    "content-type": "application/json",
                    "content-length": body.length.toString(10),
                },
                body
            });

            if (res.status === 201) {
                let json = await res.json();

                form.reset();

                set_is_open(false);

                on_added(json);
            } else if (res.status !== 500) {
                let json = await res.json();

                console.warn("failed to create new client", json);
            } else {
                console.error("server error");
            }
        } catch (err) {
            console.error("failed to send new client data", err);
        }
    };

    return <Dialog open={is_open} onOpenChange={v => {
        set_is_open(v);
    }}>
        <DialogTrigger asChild>
            <Button type="button" variant="ghost">
                <Plus/> Add Client
            </Button>
        </DialogTrigger>
        <DialogContent>
            <DialogHeader>
                <DialogTitle>Add New Client</DialogTitle>
                <DialogDescription>
                    Add a new client key that will be able to perform actions on
                    your account without having to manually authenticate
                </DialogDescription>
            </DialogHeader>
            <Separator/>
            <FormProvider<NewClient> {...form} children={
                <form className="space-y-4" onSubmit={form.handleSubmit(on_submit)}>
                    <FormField control={form.control} name="name" render={({field}) => {
                        return <FormItem>
                            <FormLabel>Client Name</FormLabel>
                            <FormControl>
                                <Input type="text" {...field}/>
                            </FormControl>
                        </FormItem>
                    }}/>
                    <FormField control={form.control} name="public_key" render={({field}) => {
                        return <FormItem>
                            <FormLabel>Public Key</FormLabel>
                            <FormControl>
                                <Input type="text" {...field}/>
                            </FormControl>
                        </FormItem>
                    }}/>
                    <div className="flex flex-row flex-nowrap gap-4">
                        <Button type="submit"><Plus/> Add Client</Button>
                        <Button type="button" variant="ghost" onClick={() => {
                            set_is_open(false);
                        }}>Cancel</Button>
                    </div>
                </form>
            }/>
        </DialogContent>
    </Dialog>;
}

interface PeerListProps {
    peers: UserPeer[],
    on_delete: (id: number) => void,
}

function PeerList({peers, on_delete}: PeerListProps) {
    return <>
        {peers.map((peer) => <PeerListItem
            key={peer.id}
            peer={peer}
            on_delete={() => {
                on_delete(peer.id);
            }}
        />)}
    </>;
}

interface PeerListItemProps {
    peer: UserPeer,
    on_delete: () => void,
}

function PeerListItem({peer, on_delete}: PeerListItemProps) {
    const [loading, set_loading] = useState(false);

    const delete_peer = async () => {
        set_loading(true);

        try {
            let body = JSON.stringify({
                type: "Peer",
                id: peer.id,
            });
            let res = await fetch("/settings/peer_client", {
                method: "DELETE",
                headers: {
                    "content-type": "application/json",
                    "content-length": body.length.toString(10),
                },
                body
            });

            if (res.status === 200) {
                console.log("peer deleted");

                on_delete();
            } else {
                let json = await res.json();

                console.warn("failed to delete peer", json);
            }
        } catch (err) {
            console.error("error when trying to delete peer item", err);
        }

        set_loading(false);
    };

    let create_date = new Date(peer.created);
    let create_distance = formatDistanceToNow(create_date, {
        addSuffix: true,
        includeSeconds: true,
    });
    let update_ele = null;

    if (peer.updated != null) {
        let update_date = new Date(peer.updated);
        let update_distance = formatDistanceToNow(update_date, {
            addSuffix: true,
            includeSeconds: true,
        });

        update_ele = <span title={update_date.toString()}>Modified: {update_distance}</span>;
    }

    return <div className="rounded-lg border p-4 space-y-4">
        <div className="flex flex-row items-center justify-between">
            <div className="flex flex-row items-center gap-x-2 grow">
                <h3 className="text-lg">{peer.name}</h3>
                /
                <span className="text-sm">{peer.addr}</span>
                /
                <span className="text-sm">{peer.port}</span>
            </div>
            <Button
                type="button"
                size="icon"
                variant="destructive"
                disabled={loading}
                onClick={() => delete_peer()}
            >
                <Trash/>
            </Button>
        </div>
        <div className="flex flex-col gap-y-1">
            <div className="flex flex-row gap-x-4">
                <div className="flex flex-row gap-x-2">
                    <Checkbox checked={peer.secure} disabled/>
                    <span className="leading-none">Secure</span>
                </div>
                <div className="flex flex-row gap-x-2">
                    <Checkbox checked={peer.ssc} disabled/>
                    <span className="leading-none">Self-Signed Certificate</span>
                </div>
            </div>
            <span>Public Key: {peer.public_key}</span>
            <span title={create_date.toString()}>Created: {create_distance}</span>
            {update_ele}
        </div>
    </div>;
}

interface NewPeer {
    name: string,
    public_key: string,
    addr: string,
    port: number,
    secure: boolean,
    ssc: boolean
}

interface AddPeerProps {
    on_added: (peer: UserPeer) => void
}

function AddPeer({on_added}: AddPeerProps) {
    const [is_open, set_is_open] = useState(false);

    const form = useForm<NewPeer>({
        defaultValues: {
            name: "",
            public_key: "",
            addr: "",
            port: 8080,
            secure: true,
            ssc: false,
        }
    });

    const on_submit: SubmitHandler<NewPeer> = async (data, event) => {
        try {
            let body = JSON.stringify({
                type: "Peer",
                ...data,
            });
            let res = await fetch("/settings/peer_client", {
                method: "POST",
                headers: {
                    "content-type": "application/json",
                    "content-length": body.length.toString(10),
                },
                body
            });

            if (res.status === 201) {
                let json = await res.json();

                form.reset();

                set_is_open(false);

                on_added(json);
            } else if (res.status !== 500) {
                let json = await res.json();

                console.warn("failed to create new peer", json);
            } else {
                console.error("server error");
            }
        } catch (err) {
            console.error("failed to send new peer data", err);
        }
    };

    return <Dialog open={is_open} onOpenChange={v => {
        set_is_open(v);
    }}>
        <DialogTrigger asChild>
            <Button type="button" variant="ghost">
                <Plus/> Add Peer
            </Button>
        </DialogTrigger>
        <DialogContent>
            <DialogHeader>
                <DialogTitle>Add New Peer</DialogTitle>
                <DialogDescription>
                    Add a new peer key that will allow this server to send data
                    on your behalf.
                </DialogDescription>
            </DialogHeader>
            <Separator/>
            <FormProvider<NewPeer> {...form} children={
                <form className="space-y-4" onSubmit={form.handleSubmit(on_submit)}>
                    <FormField control={form.control} name="name" render={({field}) => {
                        return <FormItem>
                            <FormLabel>Peer Name</FormLabel>
                            <FormControl>
                                <Input type="text" {...field}/>
                            </FormControl>
                        </FormItem>
                    }}/>
                    <div className="flex flex-row flex-nowrap items-center gap-4">
                        <FormField control={form.control} name="addr" render={({field}) => {
                            return <FormItem className="grow">
                                <FormLabel>Address</FormLabel>
                                <FormControl>
                                    <Input type="text" {...field}/>
                                </FormControl>
                            </FormItem>
                        }}/>
                        <FormField control={form.control} name="port" render={({field}) => {
                            return <FormItem className="w-1/4">
                                <FormLabel>Port</FormLabel>
                                <FormControl>
                                    <Input
                                        type="number"
                                        {...field}
                                        onChange={ev => field.onChange(parseInt(ev.target.value, 10))}
                                    />
                                </FormControl>
                            </FormItem>
                        }}/>
                    </div>
                    <FormField control={form.control} name="secure" render={({field}) => {
                        return <FormItem className="flex flex-row items-start space-x-2 space-y-0">
                            <FormControl>
                                <Checkbox checked={field.value} onCheckedChange={field.onChange}/>
                            </FormControl>
                            <div className="space-y-1 leading-none">
                                <FormLabel>Secure Connection</FormLabel>
                            </div>
                        </FormItem>
                    }}/>
                    <FormField control={form.control} name="ssc" render={({field}) => {
                        return <FormItem className="flex flex-row items-start space-x-2 space-y-0">
                            <FormControl>
                                <Checkbox checked={field.value} onCheckedChange={field.onChange}/>
                            </FormControl>
                            <div className="space-y-1 leading-none">
                                <FormLabel>Self-Signed Certificate</FormLabel>
                            </div>
                        </FormItem>
                    }}/>
                    <FormField control={form.control} name="public_key" render={({field}) => {
                        return <FormItem>
                            <FormLabel>Public Key</FormLabel>
                            <FormControl>
                                <Input type="text" {...field}/>
                            </FormControl>
                        </FormItem>
                    }}/>
                    <div className="flex flex-row flex-nowrap gap-4">
                        <Button type="submit"><Plus/> Add Peer</Button>
                        <Button type="button" variant="ghost" onClick={() => {
                            set_is_open(false);
                        }}>Cancel</Button>
                    </div>
                </form>
            }/>
        </DialogContent>
    </Dialog>;
}
