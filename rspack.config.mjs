import path from "node:path";
import { defineConfig } from "@rsbuild/core";

import { TsCheckerRspackPlugin } from "ts-checker-rspack-plugin";

const __dirname = import.meta.dirname;

const node_modules_reg = /[\\/]node_modules[\\/]/;
const react_modules_reg = /[\\/]node_modules[\\/](?:react|react-dom)[\\/]/;
const radix_modules_reg = /[\\/]node_modules[\\/]@radix-ui[\\/]/;

function chunk_test_debug(name, reg) {
    return (module) => {
        let result = reg.test(module.context);

        if (result) {
            console.log(name, module.context, result);
        }

        return result;
    };
}

// not sure why passing the regular regex does not work and this will but
// at least it will work
function context_test(reg) {
    return (module) => {
        return reg.test(module.context);
    };
}

export default defineConfig({
    mode: "development",
    entry: {
        index: "./frontend/src/index.tsx"
    },
    output: {
        path: path.resolve(__dirname, "./frontend/assets/js"),
        filename: "[name].bundle.js",
    },
    resolve: {
        tsConfig: path.resolve(__dirname, "./tsconfig.json"),
        extensions: [".ts", ".tsx", ".js"],
    },
    module: {
        rules: [
            {
                test: /\.jsx$/,
                use: {
                    loader: "builtin:swc-loader",
                    options: {
                        jsc: {
                            parser: {
                                syntax: "ecmascript",
                                jsx: true,
                            },
                            transform: {
                                react: {
                                    runtime: "automatic",
                                    //pragma: "React.createElement",
                                    //pragmaFrag: "React.Fragment",
                                    //throwIfNamespace: true,
                                    //development: false,
                                    //useBuiltins: false,
                                }
                            }
                        },
                    },
                },
                type: "javascript/auto",
            },
            {
                test: /\.tsx?$/,
                use: {
                    loader: 'builtin:swc-loader',
                    options: {
                        jsc: {
                            parser: {
                                syntax: 'typescript',
                                tsx: true,
                            },
                            transform: {
                                react: {
                                    runtime: "automatic",
                                }
                            }
                        },
                    },
                },
                type: 'javascript/auto',
            },
        ]
    },
    optimization: {
        runtimeChunk: "single",
        splitChunks: {
            cacheGroups: {
                radix: {
                    name: "radix",
                    test: context_test(radix_modules_reg),
                    priority: 20,
                    chunks: "all",
                },
                react: {
                    name: "react",
                    test: context_test(react_modules_reg),
                    priority: 20,
                    chunks: "all",
                    //reuseExistingChunk: true,
                    //minChunks: 1,
                    //minSize: 0,
                },
                vendor: {
                    name: "vendor",
                    test: context_test(node_modules_reg),
                    priority: 10,
                    chunks: "all",
                    //reuseExistingChunk: true,
                    //minChunks: 1,
                    //minSize: 0,
                }
            }
        }
    },
    plugins: [
        new TsCheckerRspackPlugin(),
    ],
    stats: "normal",
});
