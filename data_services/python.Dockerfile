FROM python:3.8.7
WORKDIR /token-server
COPY . .
RUN pip install -r requirements.txt
RUN python -m spacy download zh_core_web_sm
RUN chmod +x startup.sh
EXPOSE 8881
# Note: make sure EOL sequence is LF (Windows default is CRLF)
CMD ["./startup.sh"]