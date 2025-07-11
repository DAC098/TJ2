import { useForm, SubmitHandler } from "react-hook-form";
import { useNavigate, useLocation, Link } from "react-router-dom";

import { ApiError, req_api_json } from "@/net";

import { Input, PasswordInput } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Form, FormControl, FormField, FormItem, FormLabel, FormMessage, FormRootError } from "@/components/ui/form";
import { Separator } from "@/components/ui/separator";

interface LoginForm {
    username: string,
    password: string,
}

interface LoginSuccess {
    type: "Success"
}

interface LoginVerify {
    type: "Verify"
}

type LoginResult = LoginSuccess | LoginVerify;

export function Login() {
    const navigate = useNavigate();
    const location = useLocation();

    const form = useForm<LoginForm>({
        defaultValues: {
            username: "",
            password: "",
        }
    });

    const on_submit: SubmitHandler<LoginForm> = async (data, event) => {
        try {
            let res = await req_api_json<LoginResult>("POST", "/login", data);

            let prev = new URL(location.pathname + location.search, window.location.origin)
                .searchParams
                .get("prev");

            switch (res.type) {
                case "Success":
                    navigate(prev ?? "/journals");
                    break;
                case "Verify":
                    if (prev != null) {
                        navigate(`/verify?prev=${encodeURI(prev)}`);
                    } else {
                        navigate("/verify");
                    }
                    break;
                default:
                    console.error("unknown type from server:", res);
                    break;
            }
        } catch (err) {
            if (err instanceof ApiError) {
                switch (err.kind) {
                    case "AlreadyAuthenticated":
                        let prev = new URL(location.pathname + location.search, window.location.origin)
                            .searchParams
                            .get("prev");

                        navigate(prev ?? "/journals");
                        break;
                    case "UsernameNotFound":
                        form.setError("username", {message: "Invalid or unknown username"});
                        break;
                    case "InvalidPassword":
                        form.reset({password: ""});
                        form.setError("password", {message: "Invalid password"});
                        break;
                    case "InvalidSession":
                        form.setError("root", {message: "There was a problem with your session. Try again."});

                        document.cookie = "session_id=; max-age=0";
                        break;
                    default:
                        break;
                }
            } else {
                console.error("error when sending login:", err);

                form.setError("root", {message: "Failed to send login. client error"});
            }
        }
    };

    return <div className="flex w-full h-full">
        <div className="mx-auto my-auto border rounded-lg">
            <div className="p-4">
                <Form {...form}>
                    <form className="space-y-4" onSubmit={form.handleSubmit(on_submit)}>
                        <FormRootError/>
                        <FormField control={form.control} name="username" render={({field}) => {
                            return <FormItem>
                                <FormLabel>Username</FormLabel>
                                <FormControl>
                                    <Input type="text" {...field}/>
                                </FormControl>
                                <FormMessage/>
                            </FormItem>;
                        }}/>
                        <FormField control={form.control} name="password" render={({field}) => {
                            return <FormItem>
                                <FormLabel>Password</FormLabel>
                                <FormControl>
                                    <PasswordInput autoComplete="current-password" {...field}/>
                                </FormControl>
                                <FormMessage/>
                            </FormItem>;
                        }}/>
                        <div className="flex flex-row gap-x-4 justify-center">
                            <Button type="submit" disabled={form.formState.isSubmitting}>Login</Button>
                        </div>
                    </form>
                </Form>
            </div>
            <Separator/>
            <div className="p-4 flex flex-row gap-x-4 justify-center">
                <Link to="/register">
                    <Button type="button" variant="secondary">Register</Button>
                </Link>
            </div>
        </div>
    </div>;
}
