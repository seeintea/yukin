import OpenAI from "openai";

export const getOpenAIClient = (baseURL: string, apiKey: string) => {
  return new OpenAI({
    baseURL,
    apiKey,
    // Allow browser usage
    // Just for self usage
    dangerouslyAllowBrowser: true,
  });
};
