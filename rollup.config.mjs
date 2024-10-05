import typescript from "@rollup/plugin-typescript";
import commonjs from "@rollup/plugin-commonjs";
import replace from "@rollup/plugin-replace";
import { nodeResolve } from "@rollup/plugin-node-resolve";

export default {
    input: "./frontend/src/index.tsx",
    output: {
        file: "./frontend/assets/index.js",
        format: "umd",
        sourcemap: true,
    },
    plugins: [
        replace({
            values: {
                "process.env.NODE_ENV": JSON.stringify("development"),
            },
            preventAssignment: true,
        }),
        commonjs(),
        nodeResolve(),
        typescript({
            compilerOptions: {
                target: "ES2022",
                module: "ES2022",
                jsx: "react-jsx",
                moduleResolution: "Bundler",
                sourceMap: true,
            },
            include: [
                "./frontend/src/**/*.tsx",
                "./frontend/src/**/*.ts"
            ]
        }),
    ]
}
