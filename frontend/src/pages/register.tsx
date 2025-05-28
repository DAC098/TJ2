import { EyeOff, Eye, ArrowLeft } from "lucide-react";
import { useState, forwardRef, ComponentProps } from "react";
import { useForm, SubmitHandler } from "react-hook-form";
import { useNavigate, useLocation, Link } from "react-router-dom";

import { res_as_json } from "@/net";

import { Input, PasswordInput } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
    Form,
    FormControl,
    FormDescription,
    FormField,
    FormItem,
    FormLabel,
    FormMessage
} from "@/components/ui/form";

interface RegisterForm {
    token: string,
    username: string,
    password: string,
    confirm: string,
}

async function send_register(given: RegisterForm) {
    let body = JSON.stringify(given);
    let res = await fetch("/register", {
        method: "POST",
        headers: {
            "content-type": "application/json",
            "content-length": body.length.toString(10),
        },
        body
    });

    switch (res.status) {
        case 201:
            return true;
        default:
            return false;
    }
}

export function Register() {
    const navigate = useNavigate();
    const location = useLocation();

    const form = useForm<RegisterForm>({
        defaultValues: {
            token: "",
            username: "",
            password: "",
            confirm: "",
        }
    });

    const [sending, set_sending] = useState(false);

    const on_submit: SubmitHandler<RegisterForm> = async (data, event) => {
        set_sending(true);

        try {
            let result = await send_register(data);

            if (result) {
                navigate("/journals");
            } else {
                console.log("failed to register user");
            }
        } catch (err) {
            console.error("error when sending login:", err);

            set_sending(false);
        }
    };

    return <div className="flex w-full h-full">
        <div className="mx-auto my-auto">
            <Form {...form} children={
                <form className="space-y-4" onSubmit={form.handleSubmit(on_submit)}>
                    <FormField control={form.control} name="token" render={({field}) => {
                        return <FormItem>
                            <FormLabel>Register Token</FormLabel>
                            <FormControl>
                                <Input type="text" {...field}/>
                            </FormControl>
                        </FormItem>;
                    }}/>
                    <FormField control={form.control} name="username" render={({field}) => {
                        return <FormItem>
                            <FormLabel>Username</FormLabel>
                            <FormControl>
                                <Input type="text" {...field}/>
                            </FormControl>
                        </FormItem>;
                    }}/>
                    <FormField control={form.control} name="password" render={({field}) => {
                        return <FormItem>
                            <FormLabel>Password</FormLabel>
                            <FormControl>
                                <PasswordInput
                                    autoComplete="new-password"
                                    {...field}
                                />
                            </FormControl>
                        </FormItem>;
                    }}/>
                    <FormField control={form.control} name="confirm" render={({field}) => {
                        return <FormItem>
                            <FormLabel>Confirm Password</FormLabel>
                            <FormControl>
                                <PasswordInput
                                    autoComplete="new-password"
                                    {...field}
                                />
                            </FormControl>
                        </FormItem>;
                    }}/>
                    <div className="flex flex-row gap-x-4 justify-center">
                        <Link to="/login">
                            <Button type="button" variant="secondary"><ArrowLeft/> Login</Button>
                        </Link>
                        <Button type="submit">Sign Up</Button>
                    </div>
                </form>
            }/>
        </div>
    </div>;
}
