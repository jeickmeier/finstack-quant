import { configure } from "@testing-library/react";
import { budget } from "./budget";

// Default wait for findBy* queries and waitFor; explicit waits pass budget() themselves.
configure({ asyncUtilTimeout: budget(1000) });
