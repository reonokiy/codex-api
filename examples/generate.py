"""Generate text through a running gateway using the official OpenAI SDK."""
import os

from openai import OpenAI

with OpenAI(
    base_url=os.environ.get("OPENAI_BASE_URL", "http://127.0.0.1:8080/v1"),
    api_key=os.environ["CODEX_GATEWAY_API_KEY"],
    max_retries=0,
) as client:
    response = client.responses.create(
        model=os.environ.get("OPENAI_MODEL", "gpt-6-sol"),
        input="用一句话介绍你自己。",
    )
    print(response.output_text)
