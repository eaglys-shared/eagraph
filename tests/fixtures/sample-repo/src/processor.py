from src.models import Document, User


def process_document(doc: Document) -> dict:
    doc.validate()
    return serialize(doc)


def serialize(doc: Document) -> dict:
    return {"title": doc.title, "author": doc.author.name}


def batch_process(docs: list) -> list:
    return [process_document(d) for d in docs]
