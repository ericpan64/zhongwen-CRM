"""
Author: Martin Kess
Description: Utility functions.

Parts I changed:
    CEDICT queries adjusted to query NoSQL instead of SQL
        Added CEDICT as a parameter to adjust accordingly
    Added some function documentation
"""

# -*- coding: utf-8 -*-

''' Utility functions for rendering Chinese text. '''

from lib.stanford import segment_text, get_parts_of_speech
from flask_login import current_user
from bs4 import BeautifulSoup
from models import zwChars as z # mongoDB collection containing CEDICT dictionary


def query_cedict(phrase, as_zwPhrase=False):
    """
    :param phrase: String of the zwPhrase
    :param as_zwPhrase (optional): True if a zwPhrase object should be returned
    :return: Returns first value that exists
        - Simplified CEDICT entry
        - Traditional CEDICT entry
        - None
        By default, a CEDICT object will be returned.
    """
    simp = True
    # Query simplified words
    res = z.CEDICT.objects(simplified__phrase=phrase)

    # If no simplified found, query traditional words
    if res == None:
        simp = False
        res = z.CEDICT.objects(traditional__phrase=phrase)

    if as_zwPhrase and res != None:
        res_phrase = z.zwPhrase(phrase=phrase,pinyin=res.pinyin,definition=res.definition,is_simplified=simp)
        return res_phrase

    return res

def get_pinyin(chinese_word):
    """
    :param chinese_word: chinese word (array of 1-to-n chinese characters)
    :return: pinyin array=[word,pinyin,is_chinese]
    """
    if len(chinese_word) == 0:
        return u''

    tokens = []

    current_chinese = None    # ie. "unknown"
    current_token = chinese_word[0]

    for char in chinese_word:
        entry = query_cedict(char) # Queries simplified Chinese only
        if current_chinese is None:
            # First character
            current_chinese = entry is not None

        elif current_chinese:
            if entry is None:
                tokens.append((current_token, False))
                current_chinese = False
                current_token = char
            else:
                current_chinese = True
                current_token += char

        elif not current_chinese:
            if entry is None:
                current_token += char
                current_chinese = False
            else:
                tokens.append((current_token, False))
                current_chinese = True
                current_token = char

    if len(current_token) > 0:
        tokens.append((current_token, current_chinese))

    res = []
    for word, is_chinese in tokens:
        if is_chinese:
            # Try to get pinyin
            # "as_pymongo()[0]" returns the CEDICT object as a dict (JSON-like)
            entry = query_cedict(word).as_pymongo()[0]
            if entry is not None:
                res.append((word, entry['pinyin'], is_chinese))
            else:
                pinyin = []
                # Otherwise, just do each letter
                for char in word:
                    entry = query_cedict(word).as_pymongo()[0]
                    pinyin.append(entry['pinyin'])
                res.append((word, ' '.join(pinyin), is_chinese))
        else:
            res.append((word, word, is_chinese))

    return res

def get_tone(py):
    tones = '12345'
    for t in tones:
        if py.find(t) != -1:
            return t

    return '0'


def render_chinese_word(chinese_word,pos=''):
    """
    Generates HTML for the given Chinese word. For example:
    render_chinese_word(u'2009你好')
    <span>
        2009
    </span>
    <span class="word" tabindex="0" data-toggle="popover" data-content="/Hello!/Hey there!/" data-trigger="focus" data-original-title="你好 [ni3 hao3]" html="true">
        <span class="character tone3">你</span>
        <span class="character tone3">好</span>
    </span>

    If no definition can be found, fall back to having no tone and put an error in popover.

    The part of speech is added as the class `pos-XX` (ie. pos-NR for proper nouns) attribute
    of the word span. This way, CSS can properly mark up the text.

    :param chinese_word: Chinese word to render
    :param pos: part of speech (optional)
    :return: HTML syntax code for given Chinese
    """

    res = []
    pinyin = get_pinyin(chinese_word)

    for word, py, is_chinese in pinyin:
        if py is None:
            res.append('<span>')
            res.append(word)
            res.append('</span>')
        else:
            entry = query_cedict(word).as_pymongo()[0]

            if entry is None:
                definition = u'Could not find definition for "{}"'.format(word)
            else:
                definition = entry['definition']

            defn_html = '<ul>'
            defn_html += ''.join('<li>' + defn + '</li>' for defn in definition.split('/') if defn != '')
            defn_html += '</ul>'

            res.append(u'<span class="word pos-{} {}" tabindex="0" data-word="{}">'.format(pos, '' if is_chinese else 'non-chinese', word))
            if is_chinese:
                word_py = py.split(' ')
                assert len(word_py) == len(word), u'Pinyin mismatch - {} {}'.format(py, word)

                for char, pronunciation in zip(word, word_py):
                    res.append('<span class="character tone{}">'.format(get_tone(pronunciation)))
                    res.append(char)
                    res.append('</span>')
            else:
                for c in word:
                    res.append('<span class="character non-chinese">')
                    res.append(c)
                    res.append('</span>')

            res.append('</span>')

    return '\n'.join(res)

def generate_html(pos_text):
    """
    Takes article text, parses using BeautifulSoup for 'word' and returns HTML
    :called by: annotate_text
    :param pos_text: article text (post POS tagger)
    :returns: Single string containing segmented HTML
    """
    res = []

    bs = BeautifulSoup.BeautifulStoneSoup(pos_text)
    words = bs.sentence.findAll('word')

    for word in words:
        pos = word['pos']
        if len(word.contents) > 0:
            res.append(render_chinese_word(word.contents[0], pos))

    return ''.join(res)

def annotate_text(data):
    """
    Takes text data and converts to HTML output
    :param data: data element from EditDocumentForm()
    :returns: Segmented HTML (post POS tagger)
    """

    # Converts text to segmented HTML. Each line in data -> a paragraph
    data = data.splitlines()
    processed = []
    for line in data:
        segmented_text = segment_text(line)
        pos_text = get_parts_of_speech(segmented_text)
        processed.append(generate_html(pos_text))

    # Mark each paragraph as separate HTML segments
    res = []
    for paragraph in processed:
        res.append('<div class="paragraph">')
        res.append(paragraph)
        res.append('</div>')

    return ''.join(res)

def render_document(doc):
    """
    *esp 1/20/2020: Not in-use? Only in jinja functions and import, though no other calls in server code
    """
    bs = BeautifulSoup.BeautifulSoup(doc)
    for div in bs.findAll('div'):
        for span in div.findAll('span'):
            if 'word' in span['class'].split():
                data_word = span['data-word']
                if current_user.vocab.filter_by(simplified=data_word).count() > 0:
                    span['class'] = span.get('class', '') + ' in-vocab'
                else:
                    span['class'] = span.get('class', '') + ' not-in-vocab'

    return bs.prettify().decode('utf-8')