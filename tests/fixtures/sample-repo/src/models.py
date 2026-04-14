from typing import List, Optional


class BaseModel:
    def validate(self):
        pass


class User(BaseModel):
    def __init__(self, name: str, email: str):
        self.name = name
        self.email = email

    def validate(self):
        if not self.email:
            raise ValueError("email required")


class Document(BaseModel):
    def __init__(self, title: str, author: User):
        self.title = title
        self.author = author

    def validate(self):
        self.author.validate()
