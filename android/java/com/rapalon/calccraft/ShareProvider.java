package com.rapalon.calccraft;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;

import java.io.File;
import java.io.FileNotFoundException;

/**
 * Hands the graph's picture to the app picked in the share sheet (android/README.md,
 * "How it works"): the one file {@code cache/share/graph.png} that src/app/share.rs writes,
 * read-only, as {@code content://<application id>.share/graph.png}.
 *
 * <p>The manifest doesn't export it: only an app that Android granted the URI to (the share
 * intent's FLAG_GRANT_READ_URI_PERMISSION) can open it. It is androidx's FileProvider cut down
 * to that one file, so the app needs no library.
 */
public final class ShareProvider extends ContentProvider {
    /** The file's name, in the URI and in {@code cache/share/} (share.rs SHARED_IMAGE). */
    private static final String NAME = "graph.png";
    private static final String MIME = "image/png";

    @Override
    public boolean onCreate() {
        return true;
    }

    /** The shared file, if {@code uri} names it. */
    private File file(Uri uri) {
        if (!("/" + NAME).equals(uri.getPath())) {
            return null;
        }
        return new File(new File(getContext().getCacheDir(), "share"), NAME);
    }

    @Override
    public String getType(Uri uri) {
        return file(uri) != null ? MIME : null;
    }

    @Override
    public ParcelFileDescriptor openFile(Uri uri, String mode) throws FileNotFoundException {
        File file = file(uri);
        if (file == null) {
            throw new FileNotFoundException(uri.toString());
        }
        if (!"r".equals(mode)) {
            throw new SecurityException("Read-only: " + uri);
        }
        return ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY);
    }

    /** OpenableColumns' display name and size, which share targets show and check. */
    @Override
    public Cursor query(Uri uri, String[] projection, String selection, String[] selectionArgs,
            String sortOrder) {
        File file = file(uri);
        if (file == null) {
            return null;
        }
        if (projection == null) {
            projection = new String[] {OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE};
        }
        String[] columns = new String[projection.length];
        Object[] values = new Object[projection.length];
        int count = 0;
        for (String column : projection) {
            if (OpenableColumns.DISPLAY_NAME.equals(column)) {
                columns[count] = column;
                values[count++] = NAME;
            } else if (OpenableColumns.SIZE.equals(column)) {
                columns[count] = column;
                values[count++] = file.length();
            }
        }
        String[] usedColumns = new String[count];
        Object[] usedValues = new Object[count];
        System.arraycopy(columns, 0, usedColumns, 0, count);
        System.arraycopy(values, 0, usedValues, 0, count);
        MatrixCursor cursor = new MatrixCursor(usedColumns, 1);
        cursor.addRow(usedValues);
        return cursor;
    }

    @Override
    public Uri insert(Uri uri, ContentValues values) {
        throw new UnsupportedOperationException("Read-only");
    }

    @Override
    public int update(Uri uri, ContentValues values, String selection, String[] selectionArgs) {
        throw new UnsupportedOperationException("Read-only");
    }

    @Override
    public int delete(Uri uri, String selection, String[] selectionArgs) {
        throw new UnsupportedOperationException("Read-only");
    }
}
