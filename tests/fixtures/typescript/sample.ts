import { readFile } from "fs";

interface Config {
  name: string;
}

class Service {
  constructor(private config: Config) {}

  run(): void {
    console.log(this.config.name);
  }
}

function createService(name: string): Service {
  return new Service({ name });
}
